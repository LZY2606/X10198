use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::domain::{
    Analysis, BridgeEvidence, Candidate, CandidateAssignment, Conflict, Decision, DecisionEvent,
    EvidenceRef, GenotypeObservation, InputSnapshot, LinkPhase, PhaseBlock, ReadLink,
    Relationship, RelationshipStatus, WithdrawalEffect,
};

pub const DEFAULT_BUDGET: usize = 64;
const LOW_QUALITY: f64 = 20.0;

#[derive(Clone)]
struct Edge {
    observation_id: String,
    weight: i64,
    kind: EdgeKind,
    a: usize,
    b: usize,
    required_equal: bool,
    reason: String,
}

#[derive(Clone)]
enum EdgeKind {
    ReadLink,
    Transmission { relationship_id: String },
}

#[derive(Clone)]
struct BlockGraph {
    id: String,
    chromosome: String,
    sample_id: String,
    variant_ids: Vec<String>,
    alleles: Vec<[String; 2]>,
    selected_genotypes: Vec<String>,
    edges: Vec<Edge>,
    initial_components: Vec<usize>,
    origin_anchor: Option<OriginAnchor>,
    bridge_observation_ids: BTreeSet<String>,
}

#[derive(Clone)]
struct OriginAnchor {
    node: usize,
    relationship_id: String,
    weight: i64,
    ambiguous: bool,
    father_id: Option<String>,
    mother_id: Option<String>,
    father_allele: Option<String>,
    mother_allele: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
struct CandidateShape {
    orientation: Vec<bool>,
    swap_labels: bool,
    score: i64,
    support: Vec<EvidenceRef>,
    ignored: Vec<EvidenceRef>,
    budget_exhausted: bool,
}

pub fn analyze(
    snapshot: &InputSnapshot,
    branch_id: &str,
    events: &[DecisionEvent],
    stale_events: &[DecisionEvent],
    candidate_budget: usize,
) -> Analysis {
    let (snapshot, effective_events, withdrawal_effects) =
        apply_decisions(snapshot, events);
    let mut conflicts = collect_conflicts(&snapshot);
    let mut graphs = build_graphs(&snapshot);
    let mut blocks = Vec::new();

    for graph in graphs.drain(..) {
        let locked = locked_assignments_for(&graph, &effective_events);
        let mut candidates = enumerate_candidates(&graph, candidate_budget, &locked);
        let accepted_id = accepted_candidate_for(&graph.id, &effective_events);
        let lock_matches = locked_signature(&locked);

        for candidate in &mut candidates {
            if Some(candidate.id.clone()) == accepted_id {
                candidate.accepted = true;
            }
            if candidate.assignments.len() == lock_matches.len()
                && candidate
                    .assignments
                    .iter()
                    .all(|assignment| lock_matches.contains(assignment))
            {
                candidate.locked = true;
            }
        }

        let mut bridge_evidence = Vec::new();
        for edge in &graph.edges {
            if let EdgeKind::ReadLink = edge.kind {
                if graph.initial_components[edge.a] != graph.initial_components[edge.b] {
                    let mut left = graph.initial_components[edge.a];
                    let mut right = graph.initial_components[edge.b];
                    if left > right {
                        std::mem::swap(&mut left, &mut right);
                    }
                    bridge_evidence.push(BridgeEvidence {
                        read_link_id: edge.observation_id.clone(),
                        from_component: format!("{}:c{}", graph.id, left),
                        to_component: format!("{}:c{}", graph.id, right),
                    });
                }
            }
        }

        if let Some(reason) = blocked_decision_reason(&graph, &effective_events, &candidates) {
            conflicts.push(reason);
        }

        blocks.push(PhaseBlock {
            id: graph.id,
            chromosome: graph.chromosome,
            sample_id: graph.sample_id,
            variant_ids: graph.variant_ids,
            bridge_evidence,
            candidates,
        });
    }

    blocks.sort_by(|a, b| {
        a.sample_id
            .cmp(&b.sample_id)
            .then(a.chromosome.cmp(&b.chromosome))
            .then(
                a.variant_ids
                    .first()
                    .unwrap_or(&String::new())
                    .cmp(b.variant_ids.first().unwrap_or(&String::new())),
            )
    });
    conflicts.sort_by(|a, b| a.id.cmp(&b.id));

    Analysis {
        pinned_version: snapshot.version.id.clone(),
        branch_id: branch_id.to_string(),
        candidate_budget,
        samples: snapshot.samples,
        variants: snapshot.variants,
        genotypes: snapshot.genotypes,
        read_links: snapshot.read_links,
        relationships: snapshot.relationships,
        blocks,
        conflicts,
        effective_events,
        stale_events: stale_events.to_vec(),
        withdrawal_effects,
    }
}

fn apply_decisions(
    snapshot: &InputSnapshot,
    events: &[DecisionEvent],
) -> (
    InputSnapshot,
    Vec<DecisionEvent>,
    Vec<WithdrawalEffect>,
) {
    let mut current = snapshot.clone();
    let mut effective = Vec::new();
    let mut withdrawals = Vec::new();

    for event in events {
        if event.pinned_version != snapshot.version.id {
            continue;
        }
        match &event.decision {
            Decision::MarkRelationshipUncertain { relationship_id, .. } => {
                if let Some(relationship) = current
                    .relationships
                    .iter_mut()
                    .find(|item| item.id == *relationship_id)
                {
                    relationship.status = RelationshipStatus::Pending;
                }
            }
            Decision::ReviseRelationship {
                relationship_id,
                father_id,
                mother_id,
                status,
                ..
            } => {
                if let Some(relationship) = current
                    .relationships
                    .iter_mut()
                    .find(|item| item.id == *relationship_id)
                {
                    relationship.father_id = father_id.clone();
                    relationship.mother_id = mother_id.clone();
                    relationship.status = status.clone();
                }
            }
            Decision::WithdrawReadLink { read_link_id, .. } => {
                if let Some(link) = current
                    .read_links
                    .iter()
                    .find(|item| item.id == *read_link_id)
                {
                    let effect = withdrawal_effect(&current, link);
                    current.read_links.retain(|item| item.id != *read_link_id);
                    withdrawals.push(effect);
                }
            }
            Decision::AcceptCandidate { .. }
            | Decision::LockPhase { .. }
            | Decision::RollbackMarker { .. } => {}
        }
        effective.push(event.clone());
    }

    (current, effective, withdrawals)
}

fn build_graphs(snapshot: &InputSnapshot) -> Vec<BlockGraph> {
    let variant_by_id = snapshot
        .variants
        .iter()
        .map(|variant| (variant.id.as_str(), variant))
        .collect::<HashMap<_, _>>();
    let selected = selected_genotypes(snapshot);
    let mut groups: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();

    for ((sample_id, variant_id), genotype) in &selected {
        if genotype.alleles.as_ref().is_some_and(|alleles| is_heterozygous(alleles)) {
            let variant = variant_by_id[variant_id.as_str()];
            groups
                .entry((sample_id.clone(), variant.chromosome.clone()))
                .or_default()
                .push(variant_id.clone());
        }
    }

    let mut raw_graphs = Vec::new();
    for ((sample_id, chromosome), mut variant_ids) in groups {
        variant_ids.sort_by_key(|variant_id| {
            variant_by_id
                .get(variant_id.as_str())
                .map(|variant| variant.position)
                .unwrap_or_default()
        });
        let mut alleles = Vec::new();
        let mut selected_genotypes = Vec::new();
        let mut nodes = HashMap::new();
        for (index, variant_id) in variant_ids.iter().enumerate() {
            let genotype = selected[&(sample_id.clone(), variant_id.clone())].clone();
            let mut genotype_alleles = genotype.alleles.clone().unwrap_or_default();
            genotype_alleles.sort();
            alleles.push([
                genotype_alleles[0].clone(),
                genotype_alleles[1].clone(),
            ]);
            selected_genotypes.push(genotype.id);
            nodes.insert(variant_id.as_str(), index);
        }

        let mut edges = Vec::new();
        let mut origin_anchor = None;
        for link in &snapshot.read_links {
            if link.sample_id != sample_id {
                continue;
            }
            let variant1 = match variant_by_id.get(link.variant1_id.as_str()) {
                Some(variant) if variant.chromosome == chromosome => variant,
                _ => continue,
            };
            let variant2 = match variant_by_id.get(link.variant2_id.as_str()) {
                Some(variant) if variant.chromosome == chromosome => variant,
                _ => continue,
            };
            if !selected.contains_key(&(sample_id.clone(), variant1.id.clone()))
                || !selected.contains_key(&(sample_id.clone(), variant2.id.clone()))
            {
                continue;
            }
            let a = nodes[variant1.id.as_str()];
            let b = nodes[variant2.id.as_str()];
            if a == b {
                continue;
            }
            edges.push(Edge {
                observation_id: link.id.clone(),
                weight: link.weight,
                kind: EdgeKind::ReadLink,
                a,
                b,
                required_equal: link.phase == LinkPhase::Cis,
                reason: format!("读段连接判定为 {:?}", link.phase),
            });
        }

        for relationship in active_relationships(snapshot) {
            if relationship.child_id == sample_id && origin_anchor.is_none() {
                origin_anchor = transmission_anchor(
                    snapshot,
                    &selected,
                    &variant_by_id,
                    &nodes,
                    relationship,
                    &chromosome,
                );
            }
        }

        let initial_components = connected_components(variant_ids.len(), &edges, false);
        let bridge_observation_ids = bridge_observation_ids(variant_ids.len(), &edges);
        let id = format!(
            "block-{}-{}",
            sample_id,
            variant_ids.first().cloned().unwrap_or_else(|| chromosome.clone())
        );
        raw_graphs.push(BlockGraph {
            id,
            chromosome,
            sample_id,
            variant_ids,
            alleles,
            selected_genotypes,
            edges,
            initial_components,
            origin_anchor,
            bridge_observation_ids,
        });
    }

    let mut graphs = Vec::new();
    for graph in raw_graphs {
        if graph.variant_ids.len() <= 1 {
            graphs.push(graph);
            continue;
        }
        let component_ids = connected_components(graph.variant_ids.len(), &graph.edges, false);
        let mut component_order: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (node, component) in component_ids.iter().enumerate() {
            component_order.entry(*component).or_default().push(node);
        }
        if component_order.len() == 1 {
            graphs.push(graph);
            continue;
        }
        for nodes in component_order.into_values() {
            let mapping: HashMap<usize, usize> = nodes
                .iter()
                .enumerate()
                .map(|(new, old)| (*old, new))
                .collect();
            let edges = graph
                .edges
                .iter()
                .filter(|edge| mapping.contains_key(&edge.a) && mapping.contains_key(&edge.b))
                .cloned()
                .map(|mut edge| {
                    edge.a = mapping[&edge.a];
                    edge.b = mapping[&edge.b];
                    edge
                })
                .collect();
            let origin_anchor = graph.origin_anchor.clone().and_then(|mut anchor| {
                mapping.get(&anchor.node).map(|node| {
                    anchor.node = *node;
                    anchor
                })
            });
            graphs.push(BlockGraph {
                id: format!(
                    "block-{}-{}",
                    graph.sample_id,
                    graph.variant_ids[nodes[0]]
                ),
                chromosome: graph.chromosome.clone(),
                sample_id: graph.sample_id.clone(),
                variant_ids: nodes.iter().map(|node| graph.variant_ids[*node].clone()).collect(),
                alleles: nodes.iter().map(|node| graph.alleles[*node].clone()).collect(),
                selected_genotypes: nodes
                    .iter()
                    .map(|node| graph.selected_genotypes[*node].clone())
                    .collect(),
                edges,
                initial_components: vec![0; nodes.len()],
                origin_anchor,
                bridge_observation_ids: graph.bridge_observation_ids.clone(),
            });
        }
    }

    graphs
}

fn bridge_observation_ids(node_count: usize, edges: &[Edge]) -> BTreeSet<String> {
    let mut parent: Vec<usize> = (0..node_count).collect();
    fn find(parent: &mut [usize], value: usize) -> usize {
        if parent[value] != value {
            parent[value] = find(parent, parent[value]);
        }
        parent[value]
    }
    let mut bridges = BTreeSet::new();
    for edge in edges {
        let left = find(&mut parent, edge.a);
        let right = find(&mut parent, edge.b);
        if left != right {
            bridges.insert(edge.observation_id.clone());
            parent[left] = right;
        }
    }
    bridges
}

fn selected_genotypes(
    snapshot: &InputSnapshot,
) -> BTreeMap<(String, String), GenotypeObservation> {
    let mut by_key: BTreeMap<(String, String), Vec<GenotypeObservation>> = BTreeMap::new();
    for genotype in &snapshot.genotypes {
        by_key
            .entry((genotype.sample_id.clone(), genotype.variant_id.clone()))
            .or_default()
            .push(genotype.clone());
    }
    by_key
        .into_iter()
        .filter_map(|(key, mut values)| {
            values.sort_by(|a, b| {
                b.quality
                    .partial_cmp(&a.quality)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.id.cmp(&b.id))
            });
            values.into_iter().next().map(|value| (key, value))
        })
        .collect()
}

fn active_relationships(snapshot: &InputSnapshot) -> Vec<&Relationship> {
    snapshot
        .relationships
        .iter()
        .filter(|relationship| relationship.status != RelationshipStatus::Withdrawn)
        .collect()
}

fn transmission_anchor(
    snapshot: &InputSnapshot,
    selected: &BTreeMap<(String, String), GenotypeObservation>,
    variant_by_id: &HashMap<&str, &crate::domain::Variant>,
    child_nodes: &HashMap<&str, usize>,
    relationship: &Relationship,
    chromosome: &str,
) -> Option<OriginAnchor> {
    let mut node_variants: Vec<(&&str, &usize)> = child_nodes.iter().collect();
    node_variants.sort_by_key(|(_, node)| **node);
    for (child_variant_id, child_index) in node_variants {
        let child_variant = variant_by_id[*child_variant_id];
        if child_variant.chromosome != chromosome {
            continue;
        }
        let child_genotype = selected
            .get(&(relationship.child_id.clone(), child_variant.id.clone()))?;
        let child_alleles = child_genotype.alleles.as_ref()?;
        if !is_heterozygous(child_alleles) {
            continue;
        }
        let mut sorted_child = child_alleles.clone();
        sorted_child.sort();

        if let (Some(father_id), Some(mother_id)) =
            (relationship.father_id.as_ref(), relationship.mother_id.as_ref())
        {
            let father = selected.get(&(father_id.clone(), child_variant.id.clone()));
            let mother = selected.get(&(mother_id.clone(), child_variant.id.clone()));
            let father_alleles = father.and_then(|genotype| genotype.alleles.as_ref());
            let mother_alleles = mother.and_then(|genotype| genotype.alleles.as_ref());
            if let (Some(father_alleles), Some(mother_alleles)) = (father_alleles, mother_alleles) {
                let possibilities: Vec<(String, String)> = father_alleles
                    .iter()
                    .flat_map(|paternal| {
                        mother_alleles
                            .iter()
                            .map(move |maternal| (paternal.clone(), maternal.clone()))
                    })
                    .filter(|(paternal, maternal)| {
                        sorted_child.contains(paternal)
                            && sorted_child.contains(maternal)
                            && paternal != maternal
                    })
                    .collect();
                if possibilities.is_empty() {
                    continue;
                }
                let first = possibilities[0].clone();
                return Some(OriginAnchor {
                    node: **child_index,
                    relationship_id: relationship.id.clone(),
                    weight: relationship.weight.max(1),
                    ambiguous: possibilities.len() > 1,
                    father_id: relationship.father_id.clone(),
                    mother_id: relationship.mother_id.clone(),
                    father_allele: Some(first.0),
                    mother_allele: Some(first.1),
                });
            }
        }

        if let Some(parent_id) = relationship
            .father_id
            .clone()
            .or_else(|| relationship.mother_id.clone())
        {
            let parent = selected.get(&(parent_id.clone(), child_variant.id.clone()));
            if let Some(parent_alleles) = parent.and_then(|genotype| genotype.alleles.as_ref()) {
                let inherited: Vec<&String> = sorted_child
                    .iter()
                    .filter(|allele| parent_alleles.contains(allele))
                    .collect();
                if inherited.is_empty() {
                    continue;
                }
                let is_father = relationship.father_id.as_deref() == Some(parent_id.as_str());
                return Some(OriginAnchor {
                    node: **child_index,
                    relationship_id: relationship.id.clone(),
                    weight: relationship.weight.max(1),
                    ambiguous: inherited.len() > 1,
                    father_id: if is_father {
                        Some(parent_id.clone())
                    } else {
                        relationship.father_id.clone()
                    },
                    mother_id: if is_father {
                        relationship.mother_id.clone()
                    } else {
                        Some(parent_id.clone())
                    },
                    father_allele: if is_father {
                        Some(inherited[0].clone())
                    } else {
                        None
                    },
                    mother_allele: if is_father {
                        None
                    } else {
                        Some(inherited[0].clone())
                    },
                });
            }
        }
    }
    None
}

fn origin_anchor_evidence(anchor: &OriginAnchor) -> EvidenceRef {
    EvidenceRef {
        observation_id: format!("transmission-{}-anchor", anchor.relationship_id),
        weight: anchor.weight,
        reason: if anchor.ambiguous {
            "父源与母源分配存在两种等价标签".to_string()
        } else {
            "Mendel 传递确定来源标签".to_string()
        },
    }
}

}

fn connected_components(node_count: usize, edges: &[Edge], include_transmission: bool) -> Vec<usize> {
    let mut parent: Vec<usize> = (0..node_count).collect();
    fn find(parent: &mut [usize], value: usize) -> usize {
        if parent[value] != value {
            parent[value] = find(parent, parent[value]);
        }
        parent[value]
    }
    for edge in edges {
        if !include_transmission && matches!(edge.kind, EdgeKind::Transmission { .. }) {
            continue;
        }
        let left = find(&mut parent, edge.a);
        let right = find(&mut parent, edge.b);
        if left != right {
            parent[left] = right;
        }
    }
    (0..node_count)
        .map(|index| find(&mut parent, index))
        .collect()
}

fn enumerate_candidates(
    graph: &BlockGraph,
    budget: usize,
    locked: &BTreeMap<String, CandidateAssignment>,
) -> Vec<Candidate> {
    let n = graph.variant_ids.len();
    if n == 0 {
        return Vec::new();
    }
    let max_orientation = if n == 1 {
        1
    } else {
        2usize.saturating_pow((n - 1) as u32)
    };
    let label_options = if graph.origin_anchor.is_some() || n == 1 { 1 } else { 2 };
    let mut budget_exhausted = false;
    let mut shapes = Vec::new();
    let mut examined = 0usize;

    for mask in 0..max_orientation {
        let mut orientation = vec![false; n];
        for index in 1..n {
            orientation[index] = (mask >> (index - 1)) & 1 == 1;
        }
        for swap_labels in [false, true].into_iter().take(label_options) {
            if examined >= budget {
                budget_exhausted = true;
                break;
            }
            examined += 1;
            let shape = score_shape(graph, &orientation, budget_exhausted);
            if !violates_lock(graph, &shape, locked) {
                shapes.push(shape);
            }
        }
        if budget_exhausted {
            break;
        }
    }

    if shapes.is_empty() {
        if budget_exhausted {
            return vec![budget_warning_candidate(graph, budget)];
        }
        return Vec::new();
    }

    let best = shapes.iter().map(|shape| shape.score).max().unwrap_or(0);
    let mut best_shapes: Vec<CandidateShape> = shapes
        .into_iter()
        .filter(|shape| shape.score == best)
        .collect();
    best_shapes.sort_by(|a, b| {
        a.swap_labels
            .cmp(&b.swap_labels)
            .then_with(|| a.orientation.cmp(&b.orientation))
    });
    let tied = best_shapes.len() > 1 || budget_exhausted;

    best_shapes
        .into_iter()
        .enumerate()
        .map(|(index, shape)| {
            let ordinal = shape_hash(graph, &shape.orientation);
            let suffix = if shape.swap_labels { "B" } else { "A" };
            Candidate {
                id: format!("cand-{}-{:x}-{}", graph.id, ordinal, suffix),
                block_id: graph.id.clone(),
                score: shape.score,
                tied,
                budget_exhausted: budget_exhausted || shape.budget_exhausted,
                accepted: false,
                locked: false,
                origin_label_swapped: shape.swap_labels,
                assignments: assignments(graph, &shape.orientation, shape.swap_labels),
                supporting_observations: shape.support,
                ignored_observations: shape.ignored,
            }
        })
        .collect()
}

fn score_shape(graph: &BlockGraph, orientation: &[bool], budget_exhausted: bool) -> CandidateShape {
    let mut score = 0;
    let mut support = Vec::new();
    let mut ignored = Vec::new();

    for (index, genotype_id) in graph.selected_genotypes.iter().enumerate() {
        let evidence = EvidenceRef {
            observation_id: genotype_id.clone(),
            weight: 1,
            reason: "杂合位点构成该 phase block".to_string(),
        };
        score += 1;
        support.push(evidence);
    }

    for edge in &graph.edges {
        let node_equal = orientation[edge.a] == orientation[edge.b];
        let satisfied = if matches!(edge.kind, EdgeKind::Transmission { .. }) {
            false
        } else if node_equal == edge.required_equal {
            true
        } else {
            false
        };

        let evidence = EvidenceRef {
            observation_id: edge.observation_id.clone(),
            weight: edge.weight,
            reason: edge.reason.clone(),
        };
        if satisfied {
            score += edge.weight;
            support.push(evidence);
        } else {
            score -= edge.weight;
            ignored.push(evidence);
        }
    }

    CandidateShape {
        orientation: orientation.to_vec(),
        swap_labels: false,
        score,
        support,
        ignored,
        budget_exhausted,
    }
}

fn assignments(
    graph: &BlockGraph,
    orientation: &[bool],
    swap_labels: bool,
) -> Vec<CandidateAssignment> {
    graph
        .variant_ids
        .iter()
        .enumerate()
        .map(|(index, variant_id)| {
            let mut first = graph.alleles[index][0].clone();
            let mut second = graph.alleles[index][1].clone();
            if orientation[index] {
                std::mem::swap(&mut first, &mut second);
            }
            if swap_labels {
                std::mem::swap(&mut first, &mut second);
            }
            let mut assignment = CandidateAssignment {
                variant_id: variant_id.clone(),
                haplotype0_allele: first,
                haplotype1_allele: second,
                origin0: None,
                origin1: None,
            };
            if let Some(anchor) = &graph.origin_anchor {
                if index == anchor.node {
                    let direct = if anchor.inherited_allele == assignment.haplotype0_allele {
                        &mut assignment.origin0
                    } else {
                        &mut assignment.origin1
                    };
                    *direct = Some(anchor.parent_id.clone());
                    let indirect = if direct.as_mut().unwrap() == &anchor.parent_id {
                        if anchor.inherited_allele == assignment.haplotype0_allele {
                            &mut assignment.origin1
                        } else {
                            &mut assignment.origin0
                        }
                    } else {
                        return vec![];
                    };
                    *indirect = anchor.other_parent_id.clone();
                }
            }
            if swap_labels {
                std::mem::swap(&mut assignment.haplotype0_allele, &mut assignment.haplotype1_allele);
                std::mem::swap(&mut assignment.origin0, &mut assignment.origin1);
            }
            assignment
        })
        .collect()
}

fn violates_lock(
    graph: &BlockGraph,
    shape: &CandidateShape,
    locked: &BTreeMap<String, CandidateAssignment>,
) -> bool {
    if locked.is_empty() {
        return false;
    }
    let candidate = assignments(graph, &shape.orientation, shape.swap_labels);
    candidate
        .iter()
        .any(|assignment| locked.get(&assignment.variant_id) != Some(assignment))
}

fn shape_hash(graph: &BlockGraph, orientation: &[bool]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in graph.id.bytes().chain([0u8]) {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for bit in orientation {
        hash ^= u64::from(*bit);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn budget_warning_candidate(graph: &BlockGraph, budget: usize) -> Candidate {
    Candidate {
        id: format!("cand-{}-budget", graph.id),
        block_id: graph.id.clone(),
        score: 0,
        tied: true,
        budget_exhausted: true,
        accepted: false,
        locked: false,
        origin_label_swapped: false,
        assignments: graph
            .variant_ids
            .iter()
            .map(|variant_id| CandidateAssignment {
                variant_id: variant_id.clone(),
                haplotype0_allele: "?".to_string(),
                haplotype1_allele: "?".to_string(),
                origin0: None,
                origin1: None,
            })
            .collect(),
        supporting_observations: Vec::new(),
        ignored_observations: vec![EvidenceRef {
            observation_id: "candidate-budget".to_string(),
            weight: budget as i64,
            reason: format!("候选预算 {} 已耗尽，不能伪装成唯一解", budget),
        }],
    }
}

fn locked_assignments_for(
    graph: &BlockGraph,
    events: &[DecisionEvent],
) -> BTreeMap<String, CandidateAssignment> {
    let mut locked = BTreeMap::new();
    for event in events {
        if let Decision::LockPhase {
            block_id,
            assignments,
        } = &event.decision
        {
            if block_id == &graph.id {
                for assignment in assignments {
                    locked.insert(
                        assignment.variant_id.clone(),
                        CandidateAssignment {
                            variant_id: assignment.variant_id.clone(),
                            haplotype0_allele: assignment.haplotype0_allele.clone(),
                            haplotype1_allele: assignment.haplotype1_allele.clone(),
                            origin0: None,
                            origin1: None,
                        },
                    );
                }
            }
        }
    }
    locked
}

fn locked_signature(
    locked: &BTreeMap<String, CandidateAssignment>,
) -> Vec<CandidateAssignment> {
    locked.values().cloned().collect()
}

fn accepted_candidate_for(block_id: &str, events: &[DecisionEvent]) -> Option<String> {
    events.iter().rev().find_map(|event| match &event.decision {
        Decision::AcceptCandidate {
            block_id: event_block,
            candidate_id,
        } if event_block == block_id => Some(candidate_id.clone()),
        _ => None,
    })
}

fn blocked_decision_reason(
    graph: &BlockGraph,
    events: &[DecisionEvent],
    candidates: &[Candidate],
) -> Option<Conflict> {
    for event in events.iter().rev() {
        match &event.decision {
            Decision::AcceptCandidate {
                block_id,
                candidate_id,
            } if block_id == &graph.id => {
                if candidates.iter().any(|candidate| {
                    candidate.id == *candidate_id
                        && (!candidate.tied || candidate.accepted)
                        && !candidate.budget_exhausted
                }) {
                    return None;
                }
                if !candidates.iter().any(|candidate| candidate.id == *candidate_id) {
                    return Some(Conflict {
                        id: format!("stale-accept-{}", event.id),
                        severity: "warning".to_string(),
                        kind: "stale_decision".to_string(),
                        message: "已接受候选已因证据撤回或关系修订失效".to_string(),
                        involved_observations: vec![candidate_id.clone()],
                        minimum_restoring_sets: vec![vec!["revisit-candidate".to_string()]],
                    });
                }
            }
            Decision::LockPhase { block_id, .. } if block_id == &graph.id => {
                if candidates.iter().any(|candidate| candidate.locked) {
                    return None;
                }
                return Some(Conflict {
                    id: format!("broken-lock-{}", event.id),
                    severity: "warning".to_string(),
                    kind: "broken_lock".to_string(),
                    message: "锁定局部相位与当前证据冲突".to_string(),
                    involved_observations: vec![event.id.clone()],
                    minimum_restoring_sets: vec![vec!["revise-lock".to_string()]],
                });
            }
            _ => {}
        }
    }
    None
}

fn collect_conflicts(snapshot: &InputSnapshot) -> Vec<Conflict> {
    let selected = selected_genotypes(snapshot);
    let variant_by_id: HashMap<&str, &crate::domain::Variant> = snapshot
        .variants
        .iter()
        .map(|variant| (variant.id.as_str(), variant))
        .collect();
    let mut conflicts = Vec::new();

    for genotype in &snapshot.genotypes {
        let variant_id = genotype.variant_id.as_str();
        if let Some(alleles) = &genotype.alleles {
            let unique: BTreeSet<&String> = alleles.iter().collect();
            if alleles.len() != 2 || unique.len() > 2 {
                conflicts.push(Conflict {
                    id: format!("triallelic-{}", genotype.id),
                    severity: "error".to_string(),
                    kind: "triallelic_genotype".to_string(),
                    message: format!("{} 不是二等位二倍体基因型", genotype.id),
                    involved_observations: vec![genotype.id.clone()],
                    minimum_restoring_sets: vec![vec![genotype.id.clone()]],
                });
                continue;
            }
            if let Some(variant) = variant_by_id.get(variant_id) {
                for allele in alleles {
                    if allele != &variant.reference && allele != &variant.alternate {
                        conflicts.push(Conflict {
                            id: format!("unknown-allele-{}", genotype.id),
                            severity: "error".to_string(),
                            kind: "unknown_allele".to_string(),
                            message: format!(
                                "{} 含变异位置未声明的等位基因 {}",
                                genotype.id, allele
                            ),
                            involved_observations: vec![genotype.id.clone()],
                            minimum_restoring_sets: vec![vec![genotype.id.clone()]],
                        });
                    }
                }
            }
            if is_heterozygous(alleles) && genotype.quality < LOW_QUALITY {
                conflicts.push(Conflict {
                    id: format!("low-quality-het-{}", genotype.id),
                    severity: "warning".to_string(),
                    kind: "low_quality_heterozygous".to_string(),
                    message: format!("{} 是低质量杂合调用，相位证据降权隔离", genotype.id),
                    involved_observations: vec![genotype.id.clone()],
                    minimum_restoring_sets: vec![vec![genotype.id.clone()]],
                });
            }
        } else {
            conflicts.push(Conflict {
                id: format!("missing-{}", genotype.id),
                severity: "warning".to_string(),
                kind: "missing_genotype".to_string(),
                message: format!("{} 缺失基因型，未作为相位节点", genotype.id),
                involved_observations: vec![genotype.id.clone()],
                minimum_restoring_sets: vec![vec![genotype.id.clone()]],
            });
        }
    }

    let mut duplicate_keys: BTreeMap<(String, String), Vec<&GenotypeObservation>> =
        BTreeMap::new();
    for genotype in &snapshot.genotypes {
        duplicate_keys
            .entry((genotype.sample_id.clone(), genotype.variant_id.clone()))
            .or_default()
            .push(genotype);
    }
    for ((sample_id, variant_id), observations) in duplicate_keys {
        if observations.len() > 1 {
            let batches: BTreeSet<String> =
                observations.iter().map(|item| item.batch.clone()).collect();
            let ids: Vec<String> = observations.iter().map(|item| item.id.clone()).collect();
            let kind = if batches.len() == 1 {
                "duplicate_sample_observation"
            } else {
                "duplicate_sample_observation"
            };
            conflicts.push(Conflict {
                id: format!("duplicate-{}-{}", sample_id, variant_id),
                severity: "warning".to_string(),
                kind: kind.to_string(),
                message: "同一样本与变异存在重复观测，仅最高质量调用进入相位图".to_string(),
                involved_observations: ids,
                minimum_restoring_sets: observations
                    .iter()
                    .map(|item| vec![item.id.clone()])
                    .collect(),
            });
        }
    }

    for relationship in &snapshot.relationships {
        if relationship.status == RelationshipStatus::Pending {
            conflicts.push(Conflict {
                id: format!("relationship-pending-{}", relationship.id),
                severity: "warning".to_string(),
                kind: "relationship_uncertain".to_string(),
                message: "父母身份待确认，传递证据仅作为候选".to_string(),
                involved_observations: vec![relationship.id.clone()],
                minimum_restoring_sets: vec![vec![relationship.id.clone()]],
            });
        }
        if relationship.father_id.is_none() && relationship.mother_id.is_none() {
            conflicts.push(Conflict {
                id: format!("relationship-unparented-{}", relationship.id),
                severity: "warning".to_string(),
                kind: "missing_parent_identity".to_string(),
                message: "关系记录没有可验证的父母标识".to_string(),
                involved_observations: vec![relationship.id.clone()],
                minimum_restoring_sets: vec![vec![relationship.id.clone()]],
            });
        }
    }

    for relationship in &snapshot.relationships {
        if relationship.status == RelationshipStatus::Withdrawn {
            continue;
        }
        for variant in &snapshot.variants {
            let child = selected.get(&(relationship.child_id.clone(), variant.id.clone()));
            let father = relationship
                .father_id
                .as_ref()
                .and_then(|parent| selected.get(&(parent.clone(), variant.id.clone())));
            let mother = relationship
                .mother_id
                .as_ref()
                .and_then(|parent| selected.get(&(parent.clone(), variant.id.clone())));
            if let Some(conflict) = mendel_conflict(
                relationship,
                variant.id.clone(),
                child,
                father,
                mother,
            ) {
                conflicts.push(conflict);
            }
        }
    }

    conflicts
}

fn mendel_conflict(
    relationship: &Relationship,
    variant_id: String,
    child: Option<&GenotypeObservation>,
    father: Option<&GenotypeObservation>,
    mother: Option<&GenotypeObservation>,
) -> Option<Conflict> {
    let child = child?;
    let child_alleles = child.alleles.as_ref()?;
    let father_alleles = father.and_then(|item| item.alleles.as_ref());
    let mother_alleles = mother.and_then(|item| item.alleles.as_ref());
    let possible = match (father_alleles, mother_alleles) {
        (Some(father), Some(mother)) => father
            .iter()
            .flat_map(|paternal| mother.iter().map(move |maternal| [paternal, maternal]))
            .any(|pair| pair.contains(&&child_alleles[0]) && pair.contains(&&child_alleles[1])),
        (Some(parent), None) | (None, Some(parent)) => {
            parent.iter().any(|allele| child_alleles.contains(allele))
        }
        (None, None) => true,
    };

    if possible {
        return None;
    }

    let mut involved = vec![child.id.clone()];
    if let Some(father) = father {
        involved.push(father.id.clone());
    }
    if let Some(mother) = mother {
        involved.push(mother.id.clone());
    }
    involved.push(relationship.id.clone());

    let parent_involved = father.is_some() || mother.is_some();
    let kind = if parent_involved {
        "de_novo_or_mendelian_inconsistency"
    } else {
        "mendelian_inconsistency"
    };
    let mut minimum_sets: Vec<Vec<String>> = involved
        .iter()
        .filter(|id| !id.starts_with("rel-") || relationship.status == RelationshipStatus::Pending)
        .map(|id| vec![id.clone()])
        .collect();
    if relationship.status == RelationshipStatus::Pending {
        minimum_sets.push(vec![relationship.id.clone()]);
    }

    Some(Conflict {
        id: format!("mendel-{}-{}", relationship.id, variant_id),
        severity: "error".to_string(),
        kind: kind.to_string(),
        message: "Mendel 传递不一致；删除整条关系不能恢复约束，已隔离为可裁定证据".to_string(),
        involved_observations: involved,
        minimum_restoring_sets: minimum_sets,
    })
}

fn is_heterozygous(alleles: &[String]) -> bool {
    alleles.len() == 2 && alleles[0] != alleles[1]
}

fn withdrawal_effect(snapshot: &InputSnapshot, link: &ReadLink) -> WithdrawalEffect {
    let variant_chromosome = snapshot
        .variants
        .iter()
        .find(|variant| variant.id == link.variant1_id || variant.id == link.variant2_id)
        .map(|variant| variant.chromosome.clone())
        .unwrap_or_default();
    let before = graph_components_with(snapshot, link.sample_id.clone(), &variant_chromosome, Some(link.id.as_str()), true);
    let after = graph_components_with(snapshot, link.sample_id.clone(), &variant_chromosome, Some(link.id.as_str()), false);
    let was_bridge = before.len() < after.len();
    WithdrawalEffect {
        read_link_id: link.id.clone(),
        was_bridge,
        affected_sample_id: link.sample_id.clone(),
        chromosome: variant_chromosome,
        current_block_ids: after,
    }
}

fn graph_components_with(
    snapshot: &InputSnapshot,
    sample_id: String,
    chromosome: &str,
    link_id: Option<&str>,
    include_target: bool,
) -> Vec<String> {
    let selected = selected_genotypes(snapshot);
    let mut nodes: Vec<String> = snapshot
        .variants
        .iter()
        .filter(|variant| variant.chromosome == chromosome)
        .filter_map(|variant| {
            let genotype = selected.get(&(sample_id.clone(), variant.id.clone()))?;
            if genotype
                .alleles
                .as_ref()
                .is_some_and(|alleles| is_heterozygous(alleles))
            {
                Some(variant.id.clone())
            } else {
                None
            }
        })
        .collect();
    nodes.sort_by_key(|variant_id| {
        snapshot
            .variants
            .iter()
            .find(|variant| variant.id == *variant_id)
            .map(|variant| variant.position)
            .unwrap_or_default()
    });
    let index_by_variant: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(index, variant_id)| (variant_id.as_str(), index))
        .collect();
    let mut parent: Vec<usize> = (0..nodes.len()).collect();
    fn find(parent: &mut [usize], value: usize) -> usize {
        if parent[value] != value {
            parent[value] = find(parent, parent[value]);
        }
        parent[value]
    }
    for link in &snapshot.read_links {
        if link.sample_id != sample_id {
            continue;
        }
        if link_id == Some(link.id.as_str()) && !include_target {
            continue;
        }
        if let (Some(a), Some(b)) = (
            index_by_variant.get(link.variant1_id.as_str()),
            index_by_variant.get(link.variant2_id.as_str()),
        ) {
            let left = find(&mut parent, *a);
            let right = find(&mut parent, *b);
            if left != right {
                parent[left] = right;
            }
        }
    }
    let components: BTreeSet<usize> = (0..nodes.len())
        .map(|index| find(&mut parent, index))
        .collect();
    components
        .into_iter()
        .map(|component| format!("block-{}-{}-c{}", sample_id, chromosome, component))
        .collect()
}
