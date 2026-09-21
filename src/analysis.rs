use crate::model::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct IgnoredObservation {
    pub id: String,
    pub reason: String,
    pub related: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RepairSet {
    pub observation_ids: Vec<String>,
    pub rationale: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Conflict {
    pub id: String,
    pub kind: String,
    pub severity: String,
    pub message: String,
    pub samples: Vec<String>,
    pub variants: Vec<String>,
    pub relationships: Vec<String>,
    pub evidence_observation_ids: Vec<String>,
    pub ignored_from_phase_ids: Vec<String>,
    pub minimum_repair_sets: Vec<RepairSet>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RelationSupport {
    pub phase_relation: String,
    pub observation_id: String,
    pub kind: String,
    pub weight: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Candidate {
    pub id: String,
    pub label: String,
    pub score: f64,
    pub assignment: BTreeMap<String, Vec<AssignmentEntry>>,
    pub relations: Vec<RelationSupport>,
    pub ignored_observation_ids: Vec<String>,
    pub tie_equivalent: bool,
    pub origin_swap_equivalent: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MergeBridge {
    pub introduced_observation_id: String,
    pub merged_block_ids: Vec<String>,
    pub sample_id: String,
    pub chrom: String,
    pub start: i64,
    pub end: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Block {
    pub id: String,
    pub chrom: String,
    pub start: i64,
    pub end: i64,
    pub sample_ids: Vec<String>,
    pub variant_ids: Vec<String>,
    pub candidates: Vec<Candidate>,
    pub candidate_budget: usize,
    pub budget_reached: bool,
    pub bridges: Vec<MergeBridge>,
    pub accepted_candidate_id: Option<String>,
    pub lock_notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WithdrawalEffect {
    pub link_id: String,
    pub reason: String,
    pub old_block_id: String,
    pub split_block_ids: Vec<String>,
    pub affected_region: Region,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Region {
    pub chrom: String,
    pub start: i64,
    pub end: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Analysis {
    pub input_version: String,
    pub blocks: Vec<Block>,
    pub conflicts: Vec<Conflict>,
    pub ignored_observations: Vec<IgnoredObservation>,
    pub withdrawals: Vec<WithdrawalEffect>,
    pub relationship_status: BTreeMap<String, RelationStatus>,
}

#[derive(Clone, Debug)]
struct VarKey {
    sample_id: String,
    variant_id: String,
}

#[derive(Clone, Debug)]
struct Edge {
    id: String,
    left: VarKey,
    right: VarKey,
    parity: bool,
    weight: f64,
    kind: String,
}

#[derive(Clone)]
struct Dsu {
    parent: BTreeMap<VarKey, VarKey>,
}

impl Dsu {
    fn new(keys: impl Iterator<Item = VarKey>) -> Self {
        Self {
            parent: keys.map(|key| (key.clone(), key)).collect(),
        }
    }

    fn find(&mut self, key: &VarKey) -> VarKey {
        let current = self.parent.get(key).cloned().unwrap_or_else(|| key.clone());
        if &current == key {
            return current;
        }
        let root = self.find(&current);
        self.parent.insert(key.clone(), root.clone());
        root
    }

    fn union(&mut self, left: &VarKey, right: &VarKey) {
        let left_root = self.find(left);
        let right_root = self.find(right);
        if left_root != right_root {
            self.parent.insert(left_root, right_root);
        }
    }
}

impl VarKey {
    fn tuple(&self) -> (&str, &str) {
        (&self.sample_id, &self.variant_id)
    }
}

impl PartialEq for VarKey {
    fn eq(&self, other: &Self) -> bool {
        self.sample_id == other.sample_id && self.variant_id == other.variant_id
    }
}

impl Eq for VarKey {}

impl Ord for VarKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.tuple().cmp(&other.tuple())
    }
}

impl PartialOrd for VarKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::hash::Hash for VarKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.sample_id.hash(state);
        self.variant_id.hash(state);
    }
}

fn genotype<'a>(genotypes: &'a [GenotypeObs], sample_id: &str, variant_id: &str) -> Option<&'a GenotypeObs> {
    genotypes
        .iter()
        .find(|item| item.sample_id == sample_id && item.variant_id == variant_id)
}

fn allele_index(alleles: &[String], allele: &str) -> Option<u8> {
    alleles.iter().position(|item| item == allele).map(|index| index as u8)
}

fn callset(calls: &[String]) -> BTreeSet<String> {
    calls.iter().cloned().collect()
}

fn mendelian_violation(father: &[String], mother: &[String], child: &[String]) -> bool {
    father.iter().any(|paternal| {
        mother.iter().any(|maternal| {
            let possible = if paternal == maternal {
                vec![paternal.clone(), paternal.clone()]
            } else {
                vec![paternal.clone(), maternal.clone()]
            };
            callset(child) == callset(&possible)
        })
    }) == false
}

fn sorted_ids(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids.dedup();
    ids
}

fn edge_relation(edge: &Edge, allele_left: &str, allele_right: &str) -> String {
    if edge.parity {
        format!("{} ↔ opposite {}", allele_left, allele_right)
    } else {
        format!("{} ↔ same {}", allele_left, allele_right)
    }
}

fn minimum_removed_sets(edges: &[Edge]) -> Vec<BTreeSet<String>> {
    let mut cycles: Vec<BTreeSet<String>> = Vec::new();
    for start_index in 0..edges.len() {
        let start = edges[start_index].left.clone();
        let mut stack = vec![(start.clone(), false, 0usize)];
        let mut values: BTreeMap<VarKey, bool> = [(start, false)].into_iter().collect();
        let mut used: BTreeSet<String> = BTreeSet::new();
        while let Some((node, returning, next_index)) = stack.pop() {
            let adjacency: Vec<(usize, &VarKey, bool)> = edges
                .iter()
                .enumerate()
                .filter_map(|(index, edge)| {
                    if edge.left == node {
                        Some((index, &edge.right, edge.parity))
                    } else if edge.right == node {
                        Some((index, &edge.left, edge.parity))
                    } else {
                        None
                    }
                })
                .collect();
            if returning {
                if next_index >= adjacency.len() {
                    if let Some(previous) = stack.last_mut() {
                        previous.2 += 1;
                    }
                    continue;
                }
            }
            let scan_index = if returning { next_index } else { 0 };
            if scan_index >= adjacency.len() {
                if let Some(previous) = stack.last_mut() {
                    previous.2 += 1;
                }
                continue;
            }
            let (edge_index, next, parity) = adjacency[scan_index].clone();
            if !used.insert(edges[edge_index].id.clone()) {
                stack.push((node, true, scan_index + 1));
                continue;
            }
            let next_value = values[&node] ^ parity;
            match values.get(&next).copied() {
                Some(old) if old != next_value => {
                    cycles.push(used.iter().cloned().collect());
                    used.remove(&edges[edge_index].id);
                    stack.push((node, true, scan_index + 1));
                }
                Some(_) => {
                    used.remove(&edges[edge_index].id);
                    stack.push((node, true, scan_index + 1));
                }
                None => {
                    values.insert(next.clone(), next_value);
                    stack.push((node, true, scan_index + 1));
                    stack.push((next.clone(), false, 0));
                }
            }
        }
    }
    if cycles.is_empty() {
        return Vec::new();
    }
    cycles.sort_by_key(|cycle| cycle.len());
    cycles.dedup();
    let all: Vec<String> = cycles
        .iter()
        .flat_map(|cycle| cycle.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if all.len() <= 16 {
        for size in 1..=all.len() {
            let mut found = Vec::new();
            enumerate_sets(&all, size, 0, &mut BTreeSet::new(), &cycles, &mut found);
            if !found.is_empty() {
                found.sort();
                found.dedup();
                return found;
            }
        }
    }
    let mut removed = BTreeSet::new();
    for cycle in cycles {
        if cycle.iter().any(|id| removed.contains(id)) {
            continue;
        }
        if let Some(id) = cycle.into_iter().next() {
            removed.insert(id);
        }
    }
    vec![removed]
}

fn enumerate_sets(
    all: &[String],
    size: usize,
    offset: usize,
    chosen: &mut BTreeSet<String>,
    cycles: &[BTreeSet<String>],
    found: &mut Vec<BTreeSet<String>>,
) {
    if chosen.len() == size {
        if cycles.iter().all(|cycle| chosen.intersection(cycle).next().is_some()) {
            found.push(chosen.clone());
        }
        return;
    }
    if offset >= all.len() || chosen.len() + (all.len() - offset) < size {
        return;
    }
    chosen.insert(all[offset].clone());
    enumerate_sets(all, size, offset + 1, chosen, cycles, found);
    chosen.remove(&all[offset]);
    enumerate_sets(all, size, offset + 1, chosen, cycles, found);
}

fn transmission_edge(
    input: &InputSnapshot,
    relationship_item: &Relationship,
    variant: &Variant,
    parent_id: &str,
    parent_label: &str,
    edges: &mut Vec<Edge>,
    ignored: &mut Vec<IgnoredObservation>,
) {
    let Some(parent) = genotype(&input.genotypes, parent_id, &variant.id) else {
        ignored.push(IgnoredObservation {
            id: relationship_item.id.clone(),
            reason: format!("{} genotype missing", parent_label),
            related: vec![parent_id.to_string(), variant.id.clone()],
        });
        return;
    };
    let Some(child) = genotype(&input.genotypes, &relationship_item.child_id, &variant.id) else {
        ignored.push(IgnoredObservation {
            id: relationship_item.id.clone(),
            reason: "child genotype missing".to_string(),
            related: vec![relationship_item.child_id.clone(), variant.id.clone()],
        });
        return;
    };
    if parent.quality == Quality::Low {
        ignored.push(IgnoredObservation {
            id: parent.id.clone(),
            reason: "low-quality parent heterozygote excluded".to_string(),
            related: vec![parent_id.to_string(), variant.id.clone()],
        });
    }
    if child.quality == Quality::Low {
        ignored.push(IgnoredObservation {
            id: child.id.clone(),
            reason: "low-quality child heterozygote excluded".to_string(),
            related: vec![relationship_item.child_id.clone(), variant.id.clone()],
        });
    }
    if parent.calls.len() != 2
        || child.calls.len() != 2
        || parent.calls[0] == parent.calls[1]
        || child.calls[0] == child.calls[1]
        || parent.quality == Quality::Low
        || child.quality == Quality::Low
    {
        return;
    }
    let shared = callset(&parent.calls)
        .intersection(&callset(&child.calls))
        .cloned()
        .collect::<BTreeSet<_>>();
    if shared.len() != 1 {
        ignored.push(IgnoredObservation {
            id: relationship_item.id.clone(),
            reason: "transmission does not identify one shared allele".to_string(),
            related: vec![parent_id.to_string(), relationship_item.child_id.clone(), variant.id.clone()],
        });
        return;
    }
    let shared_allele = shared.into_iter().next().unwrap();
    let parent_parity = allele_index(&variant.alleles, &parent.calls[0]).unwrap_or(0)
        != allele_index(&variant.alleles, &shared_allele).unwrap_or(0);
    let child_parity = allele_index(&variant.alleles, &child.calls[0]).unwrap_or(0)
        != allele_index(&variant.alleles, &shared_allele).unwrap_or(0);
    edges.push(Edge {
        id: format!("{}:{}:{}", relationship_item.id, parent_label, variant.id),
        left: VarKey {
            sample_id: parent_id.to_string(),
            variant_id: variant.id.clone(),
        },
        right: VarKey {
            sample_id: relationship_item.child_id.clone(),
            variant_id: variant.id.clone(),
        },
        parity: parent_parity ^ child_parity,
        weight: parent.likelihood.max(0.0) + child.likelihood.max(0.0),
        kind: "transmission".to_string(),
    });
}

fn collect_edges(
    input: &InputSnapshot,
    withdrawn: &BTreeSet<String>,
    ignored: &mut Vec<IgnoredObservation>,
) -> Vec<Edge> {
    let mut edges = Vec::new();
    for link in &input.read_links {
        if withdrawn.contains(&link.id) {
            continue;
        }
        let Some(left) = genotype(&input.genotypes, &link.sample_id, &link.left_variant_id) else {
            ignored.push(IgnoredObservation {
                id: link.id.clone(),
                reason: "left genotype missing".to_string(),
                related: vec![link.sample_id.clone(), link.left_variant_id.clone()],
            });
            continue;
        };
        let Some(right) = genotype(&input.genotypes, &link.sample_id, &link.right_variant_id) else {
            ignored.push(IgnoredObservation {
                id: link.id.clone(),
                reason: "right genotype missing".to_string(),
                related: vec![link.sample_id.clone(), link.right_variant_id.clone()],
            });
            continue;
        };
        let left_index = input.variants.iter().position(|item| item.id == link.left_variant_id);
        let right_index = input.variants.iter().position(|item| item.id == link.right_variant_id);
        let (Some(left_variant), Some(right_variant)) = (
            left_index.map(|index| &input.variants[index]),
            right_index.map(|index| &input.variants[index]),
        ) else {
            ignored.push(IgnoredObservation {
                id: link.id.clone(),
                reason: "unknown variant".to_string(),
                related: vec![link.left_variant_id.clone(), link.right_variant_id.clone()],
            });
            continue;
        };
        if left_variant.chrom != right_variant.chrom
            || left.calls.len() != 2
            || right.calls.len() != 2
            || left.calls[0] == left.calls[1]
            || right.calls[0] == right.calls[1]
            || left.quality == Quality::Low
            || right.quality == Quality::Low
            || !left.calls.contains(&link.left_allele)
            || !right.calls.contains(&link.right_allele)
        {
            ignored.push(IgnoredObservation {
                id: link.id.clone(),
                reason: "read link cannot constrain phasing".to_string(),
                related: vec![left.id.clone(), right.id.clone()],
            });
            continue;
        }
        let left_parity = allele_index(&left_variant.alleles, &left.calls[0]).unwrap_or(0)
            != allele_index(&left_variant.alleles, &link.left_allele).unwrap_or(0);
        let right_parity = allele_index(&right_variant.alleles, &right.calls[0]).unwrap_or(0)
            != allele_index(&right_variant.alleles, &link.right_allele).unwrap_or(0);
        edges.push(Edge {
            id: link.id.clone(),
            left: VarKey {
                sample_id: link.sample_id.clone(),
                variant_id: link.left_variant_id.clone(),
            },
            right: VarKey {
                sample_id: link.sample_id.clone(),
                variant_id: link.right_variant_id.clone(),
            },
            parity: left_parity ^ right_parity,
            weight: link.weight,
            kind: "read_link".to_string(),
        });
    }

    for relationship_item in &input.relationships {
        if relationship_item.status == RelationStatus::Uncertain {
            ignored.push(IgnoredObservation {
                id: relationship_item.id.clone(),
                reason: "relationship pending confirmation".to_string(),
                related: vec![
                    relationship_item.father_id.clone(),
                    relationship_item.mother_id.clone(),
                    relationship_item.child_id.clone(),
                ],
            });
            continue;
        }
        for variant in &input.variants {
            transmission_edge(
                input,
                relationship_item,
                variant,
                &relationship_item.father_id,
                "father",
                &mut edges,
                ignored,
            );
            transmission_edge(
                input,
                relationship_item,
                variant,
                &relationship_item.mother_id,
                "mother",
                &mut edges,
                ignored,
            );
        }
    }
    edges.sort_by(|left, right| left.id.cmp(&right.id));
    edges.dedup_by(|left, right| left.id == right.id);
    ignored.sort_by(|left, right| left.id.cmp(&right.id).then(left.reason.cmp(&right.reason)));
    ignored.dedup_by(|left, right| left.id == right.id && left.reason == right.reason);
    edges
}

fn add_unique_conflict(conflicts: &mut Vec<Conflict>, conflict: Conflict) {
    if !conflicts.iter().any(|item| item.id == conflict.id) {
        conflicts.push(conflict);
    }
}

fn conflict_for_genotypes(
    input: &InputSnapshot,
    variant: &Variant,
    father: &GenotypeObs,
    mother: &GenotypeObs,
    child: &GenotypeObs,
    relationship_item: &Relationship,
) -> Conflict {
    let evidence = sorted_ids(vec![father.id.clone(), mother.id.clone(), child.id.clone()]);
    let samples = sorted_ids(vec![
        relationship_item.father_id.clone(),
        relationship_item.mother_id.clone(),
        relationship_item.child_id.clone(),
    ]);
    let mut minimum_repair_sets = vec![
        RepairSet {
            observation_ids: vec![child.id.clone()],
            rationale: "treat child call as de novo or genotype error".to_string(),
        },
        RepairSet {
            observation_ids: vec![father.id.clone()],
            rationale: "revise father genotype to restore Mendelian inheritance".to_string(),
        },
        RepairSet {
            observation_ids: vec![mother.id.clone()],
            rationale: "revise mother genotype to restore Mendelian inheritance".to_string(),
        },
        RepairSet {
            observation_ids: vec![relationship_item.id.clone()],
            rationale: "revise asserted pedigree relationship".to_string(),
        },
    ];
    let mut ignored = evidence.clone();
    ignored.push(relationship_item.id.clone());
    let mut message = "Mendelian inconsistency isolated as evidence".to_string();
    if relationship_item.status == RelationStatus::Uncertain {
        minimum_repair_sets = vec![RepairSet {
            observation_ids: vec![relationship_item.id.clone()],
            rationale: "confirm or revise uncertain parent identity".to_string(),
        }];
        message = "parent identity is uncertain; transmission constraint is isolated".to_string();
    }
    Conflict {
        id: format!("mendelian:{}:{}", relationship_item.id, variant.id),
        kind: "mendelian_inconsistency".to_string(),
        severity: "conflict".to_string(),
        message,
        samples,
        variants: vec![variant.id.clone()],
        relationships: vec![relationship_item.id.clone()],
        evidence_observation_ids: evidence,
        ignored_from_phase_ids: sorted_ids(ignored),
        minimum_repair_sets,
    }
}

fn detect_conflicts(input: &InputSnapshot) -> Vec<Conflict> {
    let mut conflicts = Vec::new();
    let genotype_pairs: BTreeMap<(&str, &str), Vec<&GenotypeObs>> =
        input.genotypes.iter().fold(BTreeMap::new(), |mut map, item| {
            map.entry((item.sample_id.as_str(), item.variant_id.as_str()))
                .or_default()
                .push(item);
            map
        });

    for ((sample_id, variant_id), observations) in &genotype_pairs {
        let Some(variant) = input.variants.iter().find(|item| item.id == *variant_id) else {
            continue;
        };
        let callsets: Vec<BTreeSet<String>> = observations
            .iter()
            .map(|item| callset(&item.calls))
            .collect();
        if observations.len() > 1 && callsets.windows(2).any(|window| window[0] != window[1]) {
            let counts = callsets.iter().fold(BTreeMap::new(), |mut map, set| {
                *map.entry(set).or_insert(0usize) += 1;
                map
            });
            let max = counts.values().copied().max().unwrap_or(1);
            let minimum_repair_sets = counts
                .iter()
                .filter(|(_, count)| **count == max)
                .map(|(set, _)| RepairSet {
                    observation_ids: observations
                        .iter()
                        .filter(|item| callset(&item.calls) != **set)
                        .map(|item| item.id.clone())
                        .collect(),
                    rationale: "remove discordant duplicate observations to restore one genotype".to_string(),
                })
                .collect();
            add_unique_conflict(
                &mut conflicts,
                Conflict {
                    id: format!("duplicate:{}:{}", sample_id, variant_id),
                    kind: "duplicate_sample_discordance".to_string(),
                    severity: "conflict".to_string(),
                    message: "duplicate sample observations disagree".to_string(),
                    samples: vec![sample_id.to_string()],
                    variants: vec![variant_id.to_string()],
                    relationships: Vec::new(),
                    evidence_observation_ids: observations.iter().map(|item| item.id.clone()).collect(),
                    ignored_from_phase_ids: observations.iter().map(|item| item.id.clone()).collect(),
                    minimum_repair_sets,
                },
            );
        }
        for item in observations.iter().copied() {
            let alleles = callset(&item.calls);
            if alleles.len() > 2 || item.calls.len() > 2 {
                add_unique_conflict(
                    &mut conflicts,
                    Conflict {
                        id: format!("triallelic:{}", item.id),
                        kind: "triallelic_error".to_string(),
                        severity: "conflict".to_string(),
                        message: "genotype contains more than two distinct alleles".to_string(),
                        samples: vec![sample_id.to_string()],
                        variants: vec![variant_id.to_string()],
                        relationships: Vec::new(),
                        evidence_observation_ids: vec![item.id.clone()],
                        ignored_from_phase_ids: vec![item.id.clone()],
                        minimum_repair_sets: vec![RepairSet {
                            observation_ids: vec![item.id.clone()],
                            rationale: "quarantine triallelic call and resequence or re-call".to_string(),
                        }],
                    },
                );
            }
            if item.quality == Quality::Low && item.calls.len() == 2 && item.calls[0] != item.calls[1] {
                add_unique_conflict(
                    &mut conflicts,
                    Conflict {
                        id: format!("low_het:{}", item.id),
                        kind: "low_quality_heterozygote".to_string(),
                        severity: "warning".to_string(),
                        message: "low-quality heterozygote is held out of phase constraints".to_string(),
                        samples: vec![sample_id.to_string()],
                        variants: vec![variant_id.to_string()],
                        relationships: Vec::new(),
                        evidence_observation_ids: vec![item.id.clone()],
                        ignored_from_phase_ids: vec![item.id.clone()],
                        minimum_repair_sets: vec![RepairSet {
                            observation_ids: vec![item.id.clone()],
                            rationale: "confirm or replace weak heterozygote".to_string(),
                        }],
                    },
                );
            }
            if item.calls.iter().any(|allele| !variant.alleles.contains(allele)) {
                add_unique_conflict(
                    &mut conflicts,
                    Conflict {
                        id: format!("unknown_allele:{}", item.id),
                        kind: "unknown_allele".to_string(),
                        severity: "conflict".to_string(),
                        message: "genotype calls an allele not declared at the variant".to_string(),
                        samples: vec![sample_id.to_string()],
                        variants: vec![variant_id.to_string()],
                        relationships: Vec::new(),
                        evidence_observation_ids: vec![item.id.clone()],
                        ignored_from_phase_ids: vec![item.id.clone()],
                        minimum_repair_sets: vec![RepairSet {
                            observation_ids: vec![item.id.clone()],
                            rationale: "correct genotype or variant allele definition".to_string(),
                        }],
                    },
                );
            }
        }
    }

    for relationship_item in &input.relationships {
        for variant in &input.variants {
            let mut present = Vec::new();
            for (sample_id, role) in [
                (&relationship_item.father_id, "father"),
                (&relationship_item.mother_id, "mother"),
                (&relationship_item.child_id, "child"),
            ] {
                match genotype(&input.genotypes, sample_id, &variant.id) {
                    Some(item) => present.push((sample_id.as_str(), role, item)),
                    None => add_unique_conflict(
                        &mut conflicts,
                        Conflict {
                            id: format!("missing:{}:{}:{}", relationship_item.id, variant.id, role),
                            kind: "missing_genotype".to_string(),
                            severity: "warning".to_string(),
                            message: format!("{} genotype is missing", role),
                            samples: vec![sample_id.to_string()],
                            variants: vec![variant.id.clone()],
                            relationships: vec![relationship_item.id.clone()],
                            evidence_observation_ids: Vec::new(),
                            ignored_from_phase_ids: vec![relationship_item.id.clone()],
                            minimum_repair_sets: vec![RepairSet {
                                observation_ids: Vec::new(),
                                rationale: "supply or explicitly mark missing genotype".to_string(),
                            }],
                        },
                    ),
                }
            }
            if present.len() == 3 {
                let father = present[0].2;
                let mother = present[1].2;
                let child = present[2].2;
                if mendelian_violation(&father.calls, &mother.calls, &child.calls) {
                    add_unique_conflict(
                        &mut conflicts,
                        conflict_for_genotypes(input, variant, father, mother, child, relationship_item),
                    );
                }
            }
        }
        if relationship_item.status == RelationStatus::Uncertain {
            add_unique_conflict(
                &mut conflicts,
                Conflict {
                    id: format!("relationship_uncertain:{}", relationship_item.id),
                    kind: "parent_identity_uncertain".to_string(),
                    severity: "warning".to_string(),
                    message: "sample relationship is marked pending confirmation".to_string(),
                    samples: sorted_ids(vec![
                        relationship_item.father_id.clone(),
                        relationship_item.mother_id.clone(),
                        relationship_item.child_id.clone(),
                    ]),
                    variants: Vec::new(),
                    relationships: vec![relationship_item.id.clone()],
                    evidence_observation_ids: vec![relationship_item.id.clone()],
                    ignored_from_phase_ids: vec![relationship_item.id.clone()],
                    minimum_repair_sets: vec![RepairSet {
                        observation_ids: vec![relationship_item.id.clone()],
                        rationale: "confirm relationship or record corrected parents".to_string(),
                    }],
                },
            );
        }
    }
    conflicts.sort_by(|left, right| left.id.cmp(&right.id));
    conflicts.dedup_by(|left, right| left.id == right.id);
    conflicts
}

fn conflicting_observation_ids(conflicts: &[Conflict]) -> BTreeSet<String> {
    conflicts
        .iter()
        .filter(|conflict| conflict.severity == "conflict")
        .flat_map(|conflict| conflict.ignored_from_phase_ids.iter().cloned())
        .collect()
}

fn valid_heterozygous(
    input: &InputSnapshot,
    sample_id: &str,
    variant_id: &str,
    forbidden: &BTreeSet<String>,
) -> bool {
    let Some(item) = genotype(&input.genotypes, sample_id, variant_id) else {
        return false;
    };
    let Some(variant) = input.variants.iter().find(|variant| variant.id == variant_id) else {
        return false;
    };
    item.quality == Quality::High
        && item.calls.len() == 2
        && item.calls[0] != item.calls[1]
        && item.calls.iter().all(|allele| variant.alleles.contains(allele))
        && !forbidden.contains(&item.id)
}

fn build_candidate(
    input: &InputSnapshot,
    nodes: &[VarKey],
    edges: &[Edge],
    root_value: bool,
    candidate_suffix: &str,
) -> Candidate {
    let mut values: BTreeMap<VarKey, bool> = BTreeMap::new();
    let root = nodes.iter().min().cloned().expect("nonempty block");
    values.insert(root.clone(), root_value);
    let mut queue = vec![root.clone()];
    while let Some(node) = queue.pop() {
        for edge in edges {
            let other = if edge.left == node {
                Some((edge.right.clone(), edge.parity))
            } else if edge.right == node {
                Some((edge.left.clone(), edge.parity))
            } else {
                None
            };
            if let Some((next, parity)) = other {
                let value = values[&node] ^ parity;
                if values.insert(next.clone(), value).is_none() {
                    queue.push(next);
                }
            }
        }
    }

    let mut assignment: BTreeMap<String, Vec<AssignmentEntry>> = BTreeMap::new();
    let mut node_set: BTreeSet<&VarKey> = nodes.iter().collect();
    for node in nodes {
        let Some(item) = genotype(&input.genotypes, &node.sample_id, &node.variant_id) else {
            continue;
        };
        let flip = values[node];
        assignment
            .entry(node.sample_id.clone())
            .or_default()
            .extend((0..2).map(|index| AssignmentEntry {
                sample_id: node.sample_id.clone(),
                variant_id: node.variant_id.clone(),
                allele: if index == 0 {
                    item.calls[if flip { 1 } else { 0 }].clone()
                } else {
                    item.calls[if flip { 0 } else { 1 }].clone()
                },
                haplotype: index as u8,
            }));
    }
    let relations: Vec<RelationSupport> = edges
        .iter()
        .map(|edge| {
            let left_value = values[&edge.left];
            let right_value = values[&edge.right];
            let left_call = genotype(&input.genotypes, &edge.left.sample_id, &edge.left.variant_id)
                .map(|item| {
                    item.calls[if left_value ^ edge.parity { 1 } else { 0 }].clone()
                })
                .unwrap_or_default();
            let right_call = genotype(&input.genotypes, &edge.right.sample_id, &edge.right.variant_id)
                .map(|item| item.calls[if right_value { 1 } else { 0 }].clone())
                .unwrap_or_default();
            let _ = &mut node_set;
            RelationSupport {
                phase_relation: edge_relation(edge, &left_call, &right_call),
                observation_id: edge.id.clone(),
                kind: edge.kind.clone(),
                weight: edge.weight,
            }
        })
        .collect();
    let score = relations.iter().map(|relation| relation.weight).sum::<f64>();
    Candidate {
        id: format!("{}-{}", root.variant_id, candidate_suffix),
        label: if root_value { "root allele on H1" } else { "root allele on H0" }.to_string(),
        score,
        assignment,
        relations,
        ignored_observation_ids: Vec::new(),
        tie_equivalent: true,
        origin_swap_equivalent: nodes.iter().map(|node| node.sample_id.clone()).collect::<BTreeSet<_>>().len() > 1,
    }
}

fn make_blocks(
    input: &InputSnapshot,
    mut edges: Vec<Edge>,
    conflicts: &[Conflict],
    withdrawn: &BTreeSet<String>,
    candidate_budget: usize,
) -> Vec<Block> {
    let forbidden = conflicting_observation_ids(conflicts);
    let mut ignored_by_conflict: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for conflict in conflicts {
        for id in &conflict.ignored_from_phase_ids {
            ignored_by_conflict
                .entry(id.clone())
                .or_default()
                .push(conflict.id.clone());
        }
    }
    edges.retain(|edge| {
        valid_heterozygous(input, &edge.left.sample_id, &edge.left.variant_id, &forbidden)
            && valid_heterozygous(input, &edge.right.sample_id, &edge.right.variant_id, &forbidden)
            && !forbidden.contains(&edge.id)
    });

    let keys: BTreeSet<VarKey> = edges
        .iter()
        .flat_map(|edge| [edge.left.clone(), edge.right.clone()])
        .collect();
    let mut dsu = Dsu::new(keys.iter().cloned());
    for edge in &edges {
        dsu.union(&edge.left, &edge.right);
    }
    let mut grouped: BTreeMap<VarKey, Vec<VarKey>> = BTreeMap::new();
    for key in keys {
        let root = dsu.find(&key);
        grouped.entry(root).or_default().push(key);
    }

    let mut blocks = Vec::new();
    for (_, mut nodes) in grouped {
        nodes.sort();
        let node_set: BTreeSet<&VarKey> = nodes.iter().collect();
        let block_edges: Vec<Edge> = edges
            .iter()
            .filter(|edge| node_set.contains(&edge.left) && node_set.contains(&edge.right))
            .cloned()
            .collect();
        let variant_set: BTreeSet<String> = nodes.iter().map(|node| node.variant_id.clone()).collect();
        let mut variants: Vec<&Variant> = input
            .variants
            .iter()
            .filter(|variant| variant_set.contains(&variant.id))
            .collect();
        variants.sort_by_key(|variant| (variant.chrom.clone(), variant.pos, variant.id.clone()));
        let chrom = variants.first().map(|variant| variant.chrom.clone()).unwrap_or_default();
        let start = variants.first().map(|variant| variant.pos).unwrap_or(0);
        let end = variants.last().map(|variant| variant.pos).unwrap_or(0);
        let root = nodes.iter().min().cloned().unwrap();
        let id = format!("block:{}:{}:{}", chrom, start, root.sample_id);

        let mut bridge_dsu = Dsu::new(nodes.iter().cloned());
        let mut bridges = Vec::new();
        for edge in block_edges.iter().filter(|edge| edge.kind == "read_link") {
            if bridge_dsu.find(&edge.left) != bridge_dsu.find(&edge.right) {
                bridge_dsu.union(&edge.left, &edge.right);
                let link = input.read_links.iter().find(|link| link.id == edge.id);
                if let Some(link) = link {
                    bridges.push(MergeBridge {
                        introduced_observation_id: link.id.clone(),
                        merged_block_ids: vec![
                            format!("block:{}:{}:{}", chrom, start, edge.left.sample_id),
                            format!("block:{}:{}:{}", chrom, start, edge.right.sample_id),
                        ],
                        sample_id: link.sample_id.clone(),
                        chrom: chrom.clone(),
                        start,
                        end,
                    });
                }
            }
        }

        let mut candidates = vec![
            build_candidate(input, &nodes, &block_edges, false, "a"),
            build_candidate(input, &nodes, &block_edges, true, "b"),
        ];
        candidates.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.id.cmp(&right.id))
        });
        let tied = (candidates[0].score - candidates[1].score).abs() < 1e-9;
        for candidate in &mut candidates {
            candidate.tie_equivalent = tied;
            candidate.ignored_observation_ids = block_edges
                .iter()
                .flat_map(|edge| ignored_by_conflict.get(&edge.id).cloned().unwrap_or_default())
                .chain(withdrawn.iter().cloned())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
        }
        blocks.push(Block {
            id,
            chrom,
            start,
            end,
            sample_ids: nodes.iter().map(|node| node.sample_id.clone()).collect::<BTreeSet<_>>().into_iter().collect(),
            variant_ids: variants.iter().map(|variant| variant.id.clone()).collect(),
            candidates,
            candidate_budget,
            budget_reached: candidate_budget <= 2,
            bridges,
            accepted_candidate_id: None,
            lock_notes: Vec::new(),
        });
    }
    blocks.sort_by(|left, right| {
        left.chrom
            .cmp(&right.chrom)
            .then(left.start.cmp(&right.start))
            .then(left.id.cmp(&right.id))
    });
    blocks
}

fn withdrawals_effects(
    input: &InputSnapshot,
    base_edges: Vec<Edge>,
    withdrawn: &[(String, String)],
    conflicts: &[Conflict],
) -> Vec<WithdrawalEffect> {
    let mut result = Vec::new();
    let mut active: BTreeSet<String> = BTreeSet::new();
    for (link_id, _) in withdrawn {
        let before = make_blocks(
            input,
            base_edges
                .iter()
                .filter(|edge| active.contains(&edge.id) || (!edge.id.starts_with("read:")))
                .cloned()
                .collect(),
            conflicts,
            &active.iter().cloned().collect(),
            8,
        );
        active.insert(link_id.clone());
        let after = make_blocks(
            input,
            base_edges
                .iter()
                .filter(|edge| active.contains(&edge.id) || (!edge.id.starts_with("read:")))
                .cloned()
                .collect(),
            conflicts,
            &active,
            8,
        );
        let touched_before: Vec<&Block> = before
            .iter()
            .filter(|block| block.bridges.iter().any(|bridge| bridge.introduced_observation_id == *link_id))
            .collect();
        if let Some(old) = touched_before.first() {
            let variants: BTreeSet<String> = old.variant_ids.iter().cloned().collect();
            let split = after
                .iter()
                .filter(|block| block.variant_ids.iter().any(|id| variants.contains(id)))
                .cloned()
                .collect::<Vec<_>>();
            result.push(WithdrawalEffect {
                link_id: link_id.clone(),
                reason: withdrawn
                    .iter()
                    .find(|(id, _)| id == link_id)
                    .map(|(_, reason)| reason.clone())
                    .unwrap_or_default(),
                old_block_id: old.id.clone(),
                split_block_ids: split.iter().map(|block| block.id.clone()).collect(),
                affected_region: Region {
                    chrom: old.chrom.clone(),
                    start: old.start,
                    end: old.end,
                },
            });
        }
    }
    result
}

fn parity_conflict(removed: &BTreeSet<String>, edges: &[Edge]) -> Option<Conflict> {
    if removed.is_empty() {
        return None;
    }
    let removed_edges: Vec<&Edge> = edges.iter().filter(|edge| removed.contains(&edge.id)).collect();
    let samples = sorted_ids(
        removed_edges
            .iter()
            .flat_map(|edge| [edge.left.sample_id.clone(), edge.right.sample_id.clone()])
            .collect(),
    );
    let variants = sorted_ids(
        removed_edges
            .iter()
            .flat_map(|edge| [edge.left.variant_id.clone(), edge.right.variant_id.clone()])
            .collect(),
    );
    Some(Conflict {
        id: format!("phase_constraint:{}", removed.iter().cloned().collect::<Vec<_>>().join("+")),
        kind: "phase_contradiction".to_string(),
        severity: "conflict".to_string(),
        message: "phase constraints form an odd parity cycle".to_string(),
        samples,
        variants,
        relationships: removed_edges
            .iter()
            .filter(|edge| edge.kind == "transmission")
            .map(|edge| edge.id.split(':').next().unwrap_or(&edge.id).to_string())
            .collect(),
        evidence_observation_ids: removed.iter().cloned().collect(),
        ignored_from_phase_ids: removed.iter().cloned().collect(),
        minimum_repair_sets: vec![RepairSet {
            observation_ids: removed.iter().cloned().collect(),
            rationale: "minimum observations removed to restore parity consistency".to_string(),
        }],
    })
}

pub fn analyze(
    input: &InputSnapshot,
    overlay: &DecisionOverlay,
    candidate_budget: usize,
) -> Analysis {
    let withdrawn: BTreeSet<String> = overlay.withdrawn_links.iter().map(|(id, _)| id.clone()).collect();
    let mut ignored = Vec::new();
    let base_edges = collect_edges(input, &withdrawn, &mut ignored);
    let removed_sets = minimum_removed_sets(&base_edges);
    let removed: BTreeSet<String> = removed_sets.into_iter().flatten().collect();
    if !removed.is_empty() {
        ignored.push(IgnoredObservation {
            id: removed.iter().cloned().collect::<Vec<_>>().join("+"),
            reason: "odd parity cycle; minimal repair set isolated".to_string(),
            related: Vec::new(),
        });
    }
    let mut conflicts = detect_conflicts(input);
    if let Some(conflict) = parity_conflict(&removed, &base_edges) {
        conflicts.push(conflict);
    }
    conflicts.sort_by(|left, right| left.id.cmp(&right.id));
    conflicts.dedup_by(|left, right| left.id == right.id);

    let edges = base_edges
        .iter()
        .filter(|edge| !removed.contains(&edge.id))
        .cloned()
        .collect();
    let mut blocks = make_blocks(input, edges, &conflicts, &withdrawn, candidate_budget);
    for accepted in &overlay.accepted {
        for block in &mut blocks {
            if block.id == accepted.block_id {
                block.accepted_candidate_id = Some(accepted.candidate_id.clone());
            }
        }
    }
    for lock in &overlay.locks {
        for block in &mut blocks {
            if block.chrom == lock.chrom
                && !(block.end < lock.start || block.start > lock.end)
                && lock.assignment.iter().any(|entry| {
                    block.sample_ids.contains(&entry.sample_id)
                        && block.variant_ids.contains(&entry.variant_id)
                })
            {
                block.lock_notes.push(lock.note.clone());
            }
        }
    }

    let withdrawals = withdrawals_effects(input, base_edges, &overlay.withdrawn_links, &conflicts);
    Analysis {
        input_version: input.version.clone(),
        blocks,
        conflicts,
        ignored_observations: ignored,
        withdrawals,
        relationship_status: overlay.relationship_status.clone(),
    }
}

pub fn validate_decision(input: &InputSnapshot, analysis: &Analysis, event: &DecisionEvent) -> Result<(), String> {
    match event {
        DecisionEvent::AcceptCandidate { block_id, candidate_id, .. } => {
            let block = analysis
                .blocks
                .iter()
                .find(|block| &block.id == block_id)
                .ok_or_else(|| format!("unknown block {block_id}"))?;
            if !block.candidates.iter().any(|candidate| &candidate.id == candidate_id) {
                return Err(format!("unknown candidate {candidate_id}"));
            }
        }
        DecisionEvent::LockPhase { sample_id, chrom, assignment, .. } => {
            if !input.samples.iter().any(|sample| &sample.id == sample_id) {
                return Err("unknown sample".to_string());
            }
            if !input.variants.iter().any(|variant| &variant.chrom == chrom) {
                return Err("unknown chromosome".to_string());
            }
            for entry in assignment {
                let item = genotype(&input.genotypes, &entry.sample_id, &entry.variant_id)
                    .ok_or_else(|| "locked entry has no genotype".to_string())?;
                if entry.sample_id != *sample_id || !item.calls.contains(&entry.allele) {
                    return Err("locked allele is not present".to_string());
                }
            }
        }
        DecisionEvent::WithdrawReadLink { link_id, .. } => {
            if !input.read_links.iter().any(|link| &link.id == link_id) {
                return Err("unknown read link".to_string());
            }
        }
        DecisionEvent::SetRelationshipStatus { relationship_id, .. } => {
            if !input.relationships.iter().any(|item| &item.id == relationship_id) {
                return Err("unknown relationship".to_string());
            }
        }
    }
    Ok(())
}
