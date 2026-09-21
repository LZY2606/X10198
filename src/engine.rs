use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::model::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Analysis {
    pub input: AnalysisInput,
    pub overlay: DecisionOverlay,
    pub blocks: Vec<PhaseBlock>,
    pub candidates: Vec<Candidate>,
    pub conflicts: Vec<Conflict>,
    pub warnings: Vec<Warning>,
    pub bridge_events: Vec<BridgeEvent>,
    pub budget_exhausted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRef {
    pub node_id: String,
    pub sample_id: String,
    pub variant_id: String,
    pub chromosome: String,
    pub position: i64,
    pub alleles: [String; 2],
    pub genotype_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhaseBlock {
    pub id: String,
    pub chromosome: String,
    pub nodes: Vec<NodeRef>,
    pub bridge_evidence: Vec<BridgeEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BridgeEvidence {
    pub observation_id: String,
    pub left_node: String,
    pub right_node: String,
    pub weight: f64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BridgeEvent {
    pub observation_id: String,
    pub chromosome: String,
    pub sample_id: String,
    pub kind: String,
    pub affected_region: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Candidate {
    pub id: String,
    pub block_id: String,
    pub score: f64,
    pub tied: bool,
    pub orientation: BTreeMap<String, [String; 2]>,
    pub relation_sources: Vec<RelationSource>,
    pub de_novo: Vec<DeNovoCall>,
    pub support: Vec<EvidenceRef>,
    pub ignored: Vec<IgnoredData>,
    pub budget_limited: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationSource {
    pub relationship_id: String,
    pub child_variant: String,
    pub paternal_allele: String,
    pub maternal_allele: String,
    pub weight: f64,
    pub ignored: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeNovoCall {
    pub genotype_id: String,
    pub relationship_id: Option<String>,
    pub allele: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceRef {
    pub observation_id: String,
    pub weight: f64,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IgnoredData {
    pub observation_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Conflict {
    pub id: String,
    pub kind: String,
    pub severity: String,
    pub description: String,
    pub observations: Vec<String>,
    pub minimal_repairs: Vec<RepairSet>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepairSet {
    pub observation_ids: Vec<String>,
    pub restored_constraint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Warning {
    pub id: String,
    pub kind: String,
    pub description: String,
    pub observations: Vec<String>,
}

pub fn stable_hash(value: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub fn import_to_input(mut payload: ImportPayload) -> (AnalysisInput, Vec<Conflict>) {
    let version_id = payload
        .version_id
        .take()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("v-{}", stable_hash(&serde_json::to_string(&payload).unwrap_or_default())));

    let mut samples = Vec::new();
    let mut relationships = Vec::new();
    let mut variants = Vec::new();
    let mut genotypes = Vec::new();
    let mut read_links = Vec::new();
    let mut conflicts = Vec::new();
    let mut seen_ids: BTreeMap<String, String> = BTreeMap::new();
    let mut sample_ids: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut genotype_keys: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();

    for observation in payload.observations {
        let id = observation.id().to_string();
        if let Some(first) = seen_ids.get(&id) {
            conflicts.push(Conflict {
                id: format!("conflict-duplicate-id-{}", stable_hash(&id)),
                kind: "duplicate_observation".to_string(),
                severity: "error".to_string(),
                description: format!("观测 ID {id} 在输入版本中重复"),
                observations: vec![first.clone(), id.clone()],
                minimal_repairs: vec![RepairSet {
                    observation_ids: vec![id.clone()],
                    restored_constraint: "保留首个观测后，唯一标识约束恢复".to_string(),
                }],
            });
            continue;
        }
        seen_ids.insert(id.clone(), id.clone());
        match observation {
            ObservationInput::Sample(value) => {
                sample_ids.entry(value.id.clone()).or_default().push(value.id.clone());
                samples.push(Sample {
                    id: value.id,
                    anonymous_label: value.anonymous_label,
                    batch: value.batch,
                });
            }
            ObservationInput::Relationship(value) => relationships.push(Relationship {
                id: value.id,
                child: value.child,
                father: value.father,
                mother: value.mother,
                confidence: value.confidence,
                status: value.status.unwrap_or_else(|| "confirmed".to_string()),
                batch: value.batch,
            }),
            ObservationInput::Variant(value) => variants.push(Variant {
                id: value.id,
                chromosome: value.chromosome,
                position: value.position,
                reference_allele: value.reference_allele,
                alternate_alleles: value.alternate_alleles,
                batch: value.batch,
            }),
            ObservationInput::Genotype(value) => {
                genotype_keys
                    .entry((value.sample.clone(), value.variant.clone()))
                    .or_default()
                    .push(value.id.clone());
                genotypes.push(Genotype {
                    id: value.id,
                    sample: value.sample,
                    variant: value.variant,
                    alleles: value.alleles,
                    likelihoods: value.likelihoods,
                    quality: value.quality,
                    missing: value.missing,
                    low_quality_heterozygous: value.low_quality_heterozygous,
                    duplicate_observation: value.duplicate_observation,
                    batch: value.batch,
                });
            }
            ObservationInput::ReadLink(value) => read_links.push(ReadLink {
                id: value.id,
                sample: value.sample,
                chromosome: value.chromosome,
                left_variant: value.left_variant,
                right_variant: value.right_variant,
                left_allele: value.left_allele,
                right_allele: value.right_allele,
                weight: value.weight,
                withdrawn: value.withdrawn,
                batch: value.batch,
            }),
        }
    }

    for (sample_id, observation_ids) in sample_ids {
        if observation_ids.len() > 1 {
            conflicts.push(Conflict {
                id: format!("conflict-duplicate-sample-{}", stable_hash(&sample_id)),
                kind: "duplicate_sample".to_string(),
                severity: "error".to_string(),
                description: format!("匿名样本 {sample_id} 被重复登记"),
                observations: observation_ids,
                minimal_repairs: vec![RepairSet {
                    observation_ids: vec![sample_id],
                    restored_constraint: "保留一条样本登记后，家系节点唯一".to_string(),
                }],
            });
        }
    }
    for ((sample_id, variant_id), observation_ids) in genotype_keys {
        if observation_ids.len() > 1 {
            conflicts.push(Conflict {
                id: format!(
                    "conflict-duplicate-genotype-{}-{}",
                    stable_hash(&sample_id),
                    stable_hash(&variant_id)
                ),
                kind: "duplicate_genotype".to_string(),
                severity: "warning".to_string(),
                description: format!("样本 {sample_id} 在变异 {variant_id} 上存在重复基因型观测"),
                observations: observation_ids,
                minimal_repairs: vec![RepairSet {
                    observation_ids: vec![sample_id, variant_id],
                    restored_constraint: "仅保留首个基因型用于相位，重复观测作为证据隔离".to_string(),
                }],
            });
        }
    }

    (
        AnalysisInput {
            version_id,
            note: payload.note,
            candidate_budget: payload.candidate_budget.unwrap_or(16).max(2),
            samples,
            relationships,
            variants,
            genotypes,
            read_links,
        },
        conflicts,
    )
}

pub fn analyze(input: AnalysisInput, mut overlay: DecisionOverlay, mut import_conflicts: Vec<Conflict>) -> Analysis {
    apply_overlay(&input, &mut overlay, &mut import_conflicts);
    let mut warnings = Vec::new();

    let variant_map: BTreeMap<&str, &Variant> =
        input.variants.iter().map(|item| (item.id.as_str(), item)).collect();
    let sample_set: BTreeSet<&str> = input.samples.iter().map(|item| item.id.as_str()).collect();
    let mut valid_links = Vec::new();

    for link in &input.read_links {
        if overlay.withdrawn_links.contains(&link.id) {
            warnings.push(Warning {
                id: format!("warning-withdrawn-{}", stable_hash(&link.id)),
                kind: "read_link_withdrawn".to_string(),
                description: format!("读段连接 {} 已被撤回，不参与 block 构建", link.id),
                observations: vec![link.id.clone()],
            });
            continue;
        }
        let left = variant_map.get(link.left_variant.as_str());
        let right = variant_map.get(link.right_variant.as_str());
        let valid_sample = sample_set.contains(link.sample.as_str());
        let same_chromosome = left
            .zip(right)
            .map(|(left, right)| left.chromosome == link.chromosome && right.chromosome == link.chromosome)
            .unwrap_or(false);
        let left_genotype = input.genotype(&link.sample, &link.left_variant);
        let right_genotype = input.genotype(&link.sample, &link.right_variant);
        let alleles_ok = left_genotype
            .map(|genotype| genotype.can_supply(&link.left_allele))
            .unwrap_or(false)
            && right_genotype
                .map(|genotype| genotype.can_supply(&link.right_allele))
                .unwrap_or(false);
        if !valid_sample || !same_chromosome || !alleles_ok {
            import_conflicts.push(Conflict {
                id: format!("conflict-invalid-link-{}", stable_hash(&link.id)),
                kind: "invalid_read_link".to_string(),
                severity: "error".to_string(),
                description: format!("读段连接 {} 指向缺失样本、错误染色体或不兼容等位基因", link.id),
                observations: vec![link.id.clone()],
                minimal_repairs: vec![RepairSet {
                    observation_ids: vec![link.id.clone()],
                    restored_constraint: "隔离该读段后，图边约束恢复一致".to_string(),
                }],
            });
            continue;
        }
        valid_links.push(link);
    }

    for genotype in &input.genotypes {
        if !sample_set.contains(genotype.sample.as_str()) {
            import_conflicts.push(Conflict {
                id: format!("conflict-missing-sample-genotype-{}", stable_hash(&genotype.id)),
                kind: "missing_reference".to_string(),
                severity: "error".to_string(),
                description: format!("基因型 {} 引用未知样本 {}", genotype.id, genotype.sample),
                observations: vec![genotype.id.clone()],
                minimal_repairs: vec![RepairSet {
                    observation_ids: vec![genotype.id.clone()],
                    restored_constraint: "移除悬空基因型引用后，引用完整性恢复".to_string(),
                }],
            });
        }
        if !variant_map.contains_key(genotype.variant.as_str()) {
            import_conflicts.push(Conflict {
                id: format!("conflict-missing-variant-genotype-{}", stable_hash(&genotype.id)),
                kind: "missing_reference".to_string(),
                severity: "error".to_string(),
                description: format!("基因型 {} 引用未知变异 {}", genotype.id, genotype.variant),
                observations: vec![genotype.id.clone()],
                minimal_repairs: vec![RepairSet {
                    observation_ids: vec![genotype.id.clone()],
                    restored_constraint: "移除悬空基因型引用后，引用完整性恢复".to_string(),
                }],
            });
        }
        if genotype.missing {
            warnings.push(Warning {
                id: format!("warning-missing-{}", stable_hash(&genotype.id)),
                kind: "missing_genotype".to_string(),
                description: format!("基因型 {} 缺失，不强制 Mendelian 等位基因", genotype.id),
                observations: vec![genotype.id.clone()],
            });
        }
        if genotype.low_quality_heterozygous {
            warnings.push(Warning {
                id: format!("warning-lowq-{}", stable_hash(&genotype.id)),
                kind: "low_quality_heterozygous".to_string(),
                description: format!("杂合基因型 {} 质量较低，仅作为降权候选证据", genotype.id),
                observations: vec![genotype.id.clone()],
            });
        }
        if genotype.duplicate_observation {
            warnings.push(Warning {
                id: format!("warning-duplicate-{}", stable_hash(&genotype.id)),
                kind: "duplicate_genotype".to_string(),
                description: format!("基因型 {} 标记为重复观测，已从相位图排除", genotype.id),
                observations: vec![genotype.id.clone()],
            });
        }
        if genotype.triallelic() {
            import_conflicts.push(Conflict {
                id: format!("conflict-triallelic-{}", stable_hash(&genotype.id)),
                kind: "triallelic_error".to_string(),
                severity: "error".to_string(),
                description: format!("二倍体基因型 {} 含三个等位基因 {:?}", genotype.id, genotype.alleles),
                observations: vec![genotype.id.clone()],
                minimal_repairs: vec![RepairSet {
                    observation_ids: vec![genotype.id.clone()],
                    restored_constraint: "隔离三等位观测后，二倍体节点约束恢复".to_string(),
                }],
            });
        }
    }

    for relationship in &input.relationships {
        if !sample_set.contains(relationship.child.as_str())
            || relationship.father.as_deref().map_or(false, |id| !sample_set.contains(id))
            || relationship.mother.as_deref().map_or(false, |id| !sample_set.contains(id))
        {
            import_conflicts.push(Conflict {
                id: format!("conflict-relationship-reference-{}", stable_hash(&relationship.id)),
                kind: "missing_reference".to_string(),
                severity: "error".to_string(),
                description: format!("关系 {} 引用了未知样本", relationship.id),
                observations: vec![relationship.id.clone()],
                minimal_repairs: vec![RepairSet {
                    observation_ids: vec![relationship.id.clone()],
                    restored_constraint: "隔离悬空关系后，家系引用恢复完整".to_string(),
                }],
            });
        }
        if relationship.is_uncertain() {
            warnings.push(Warning {
                id: format!("warning-uncertain-relationship-{}", stable_hash(&relationship.id)),
                kind: "parent_identity_uncertain".to_string(),
                description: format!("关系 {} 的父母身份待确认", relationship.id),
                observations: vec![relationship.id.clone()],
            });
        }
    }

    detect_mendel(&input, &mut import_conflicts);

    let mut nodes = BTreeMap::new();
    for genotype in &input.genotypes {
        if !genotype.heterozygous()
            || genotype.triallelic()
            || genotype.duplicate_observation
            || !sample_set.contains(genotype.sample.as_str())
            || !variant_map.contains_key(genotype.variant.as_str())
        {
            continue;
        }
        let variant = variant_map[genotype.variant.as_str()];
        let mut alleles = genotype.alleles.clone();
        alleles.sort();
        let alleles = [alleles[0].clone(), alleles[1].clone()];
        let node_id = format!(
            "n-{}-{}-{}",
            stable_hash(&genotype.sample),
            stable_hash(&genotype.variant),
            stable_hash(&alleles.concat())
        );
        nodes.insert(
            (genotype.sample.clone(), genotype.variant.clone()),
            NodeRef {
                node_id,
                sample_id: genotype.sample.clone(),
                variant_id: genotype.variant.clone(),
                chromosome: variant.chromosome.clone(),
                position: variant.position,
                alleles,
                genotype_id: genotype.id.clone(),
            },
        );
    }

    let mut parent = nodes
        .values()
        .map(|node| (node.node_id.clone(), node.node_id.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut bridge_events = Vec::new();

    fn find(parent: &mut BTreeMap<String, String>, value: &str) -> String {
        let mut root = value.to_string();
        while parent[&root] != root {
            root = parent[&root].clone();
        }
        let mut cursor = value.to_string();
        while parent[&cursor] != cursor {
            let next = parent[&cursor].clone();
            parent.insert(cursor.clone(), root.clone());
            cursor = next;
        }
        root
    }

    for link in &valid_links {
        if let (Some(left), Some(right)) = (
            nodes.get(&(link.sample.clone(), link.left_variant.clone())),
            nodes.get(&(link.sample.clone(), link.right_variant.clone())),
        ) {
            let left_root = find(&mut parent, &left.node_id);
            let right_root = find(&mut parent, &right.node_id);
            let _evidence = BridgeEvidence {
                observation_id: link.id.clone(),
                left_node: left.node_id.clone(),
                right_node: right.node_id.clone(),
                weight: link.weight,
                status: "active".to_string(),
            };
            if left_root != right_root {
                bridge_events.push(BridgeEvent {
                    observation_id: link.id.clone(),
                    chromosome: link.chromosome.clone(),
                    sample_id: link.sample.clone(),
                    kind: "merge".to_string(),
                    affected_region: vec![left.node_id.clone(), right.node_id.clone()],
                });
                parent.insert(left_root, right_root.clone());
            }
        }
    }

    for link in &input.read_links {
        if overlay.withdrawn_links.contains(&link.id) {
            if let (Some(left), Some(right)) = (
                nodes.get(&(link.sample.clone(), link.left_variant.clone())),
                nodes.get(&(link.sample.clone(), link.right_variant.clone())),
            ) {
                bridge_events.push(BridgeEvent {
                    observation_id: link.id.clone(),
                    chromosome: link.chromosome.clone(),
                    sample_id: link.sample.clone(),
                    kind: "withdraw_split_region".to_string(),
                    affected_region: vec![left.node_id.clone(), right.node_id.clone()],
                });
            }
        }
    }

    let mut root_members: BTreeMap<String, Vec<NodeRef>> = BTreeMap::new();
    for node in nodes.values().cloned() {
        let root = find(&mut parent, &node.node_id);
        root_members.entry(root).or_default().push(node);
    }
    let node_root = nodes
        .values()
        .map(|node| (node.node_id.clone(), find(&mut parent, &node.node_id)))
        .collect::<BTreeMap<_, _>>();
    let mut bridge_evidence: BTreeMap<String, Vec<BridgeEvidence>> = BTreeMap::new();
    for link in &valid_links {
        if let (Some(left), Some(right)) = (
            nodes.get(&(link.sample.clone(), link.left_variant.clone())),
            nodes.get(&(link.sample.clone(), link.right_variant.clone())),
        ) {
            if let Some(root) = node_root.get(&left.node_id) {
                bridge_evidence
                    .entry(root.clone())
                    .or_default()
                    .push(BridgeEvidence {
                        observation_id: link.id.clone(),
                        left_node: left.node_id.clone(),
                        right_node: right.node_id.clone(),
                        weight: link.weight,
                        status: "active".to_string(),
                    });
            }
        }
    }

    let mut blocks = Vec::new();
    for (root, mut members) in root_members {
        members.sort_by(|left, right| {
            left.chromosome
                .cmp(&right.chromosome)
                .then(left.position.cmp(&right.position))
                .then(left.sample_id.cmp(&right.sample_id))
                .then(left.variant_id.cmp(&right.variant_id))
        });
        let chromosome = members
            .first()
            .map(|node| node.chromosome.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let span = members
            .iter()
            .map(|node| format!("{}:{}", node.sample_id, node.variant_id))
            .collect::<Vec<_>>()
            .join("|");
        let mut evidence = bridge_evidence.remove(&root).unwrap_or_default();
        evidence.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
        blocks.push(PhaseBlock {
            id: format!("block-{}-{}", stable_hash(&chromosome), stable_hash(&span)),
            chromosome,
            nodes: members,
            bridge_evidence: evidence,
        });
    }
    blocks.sort_by(|left, right| {
        left.chromosome
            .cmp(&right.chromosome)
            .then(left.nodes.first().map(|n| n.position).cmp(&right.nodes.first().map(|n| n.position)))
            .then(left.id.cmp(&right.id))
    });

    let mut candidates = Vec::new();
    let mut budget_exhausted = false;
    for block in &blocks {
        let result = solve_block(&input, &overlay, block, valid_links.iter().copied().collect());
        budget_exhausted |= result.budget_exhausted;
        candidates.extend(result.candidates);
    }

    Analysis {
        input,
        overlay,
        blocks,
        candidates,
        conflicts: dedupe_conflicts(import_conflicts),
        warnings: dedupe_warnings(warnings),
        bridge_events,
        budget_exhausted,
    }
}

fn apply_overlay(input: &AnalysisInput, overlay: &mut DecisionOverlay, conflicts: &mut Vec<Conflict>) {
    let relationships: BTreeMap<&str, Relationship> = input
        .relationships
        .iter()
        .map(|item| (item.id.as_str(), item.clone()))
        .collect();
    let mut remaining = Vec::new();
    let mut withdrawn_before = BTreeSet::new();
    for decision in overlay.decisions.drain(..) {
        match &decision {
            Decision::WithdrawReadLink { read_link_id, reason } => {
                if !overlay.withdrawn_links.contains(read_link_id) {
                    overlay.withdrawn_links.push(read_link_id.clone());
                }
                withdrawn_before.insert(read_link_id.clone());
                conflicts.push(Conflict {
                    id: format!("adjudication-withdraw-{}", stable_hash(read_link_id)),
                    kind: "read_link_withdrawn".to_string(),
                    severity: "adjudication".to_string(),
                    description: reason.clone().unwrap_or_else(|| format!("读段 {read_link_id} 被人工撤回")),
                    observations: vec![read_link_id.clone()],
                    minimal_repairs: vec![RepairSet {
                        observation_ids: vec![read_link_id.clone()],
                        restored_constraint: "仅拆分该读段桥接的局部区域，其他连接保持".to_string(),
                    }],
                });
            }
            Decision::MarkRelationshipUncertain { relationship_id, note } => {
                overlay
                    .uncertain_relationships
                    .entry(relationship_id.clone())
                    .or_insert_with(|| note.clone().unwrap_or_else(|| "父母身份待确认".to_string()));
            }
            Decision::ReviseRelationship {
                relationship_id,
                father,
                mother,
                confidence,
                note,
            } => {
                overlay.revised_relationships.insert(
                    relationship_id.clone(),
                    RelationshipRevision {
                        father: father.clone(),
                        mother: mother.clone(),
                        confidence: *confidence,
                        note: note.clone(),
                    },
                );
            }
            Decision::AcceptCandidate { block_id, candidate_id } => {
                overlay
                    .accepted_candidates
                    .insert(block_id.clone(), candidate_id.clone());
            }
            Decision::LockPhase {
                block_id,
                variant_id,
                allele_a,
                allele_b,
            } => {
                overlay.locked_phases.insert(
                    format!("{block_id}:{variant_id}"),
                    LockedPhase {
                        variant_id: variant_id.clone(),
                        allele_a: allele_a.clone(),
                        allele_b: allele_b.clone(),
                    },
                );
            }
        }
        remaining.push(decision);
    }
    overlay.decisions = remaining;
    let _ = relationships;
}

struct BlockSolution {
    candidates: Vec<Candidate>,
    budget_exhausted: bool,
}

#[derive(Clone)]
struct RelationItem {
    relationship_id: String,
    node_id: String,
    child_genotype_id: String,
    father_genotype_id: Option<String>,
    mother_genotype_id: Option<String>,
    uncertain: bool,
    weight: f64,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Assignment {
    paternal_allele: String,
    maternal_allele: String,
    relationship_id: String,
    ignored: bool,
    de_novo_alleles: Vec<String>,
}

#[derive(Clone)]
struct RawCandidate {
    orientation: BTreeMap<String, u8>,
    assignments: Vec<Assignment>,
    score: f64,
}

fn solve_block<'a>(
    input: &'a AnalysisInput,
    overlay: &DecisionOverlay,
    block: &PhaseBlock,
    active_links: Vec<&'a ReadLink>,
) -> BlockSolution {
    let mut node_map = BTreeMap::new();
    let mut node_bits: BTreeMap<String, u8> = BTreeMap::new();
    for node in &block.nodes {
        node_map.insert(node.node_id.clone(), node.clone());
        node_bits.insert(node.node_id.clone(), 0);
    }

    let mut relation_items = Vec::new();
    let node_keys = block
        .nodes
        .iter()
        .map(|node| ((node.sample_id.clone(), node.variant_id.clone()), node.clone()))
        .collect::<BTreeMap<_, _>>();

    for relationship in &input.relationships {
        let revised = overlay.revised_relationships.get(&relationship.id);
        let father = revised
            .and_then(|item| item.father.clone())
            .or_else(|| relationship.father.clone());
        let mother = revised
            .and_then(|item| item.mother.clone())
            .or_else(|| relationship.mother.clone());
        let confidence = revised.map(|item| item.confidence).unwrap_or(relationship.confidence);
        let uncertain = relationship.is_uncertain()
            || overlay.uncertain_relationships.contains_key(&relationship.id)
            || confidence < 0.95;
        for ((sample_id, variant_id), node) in &node_keys {
            if sample_id != &relationship.child {
                continue;
            }
            let child = input.genotype(sample_id, variant_id);
            let father_genotype = father
                .as_deref()
                .and_then(|parent_id| input.genotype(parent_id, variant_id));
            let mother_genotype = mother
                .as_deref()
                .and_then(|parent_id| input.genotype(parent_id, variant_id));
            if let Some(child) = child {
                relation_items.push(RelationItem {
                    relationship_id: relationship.id.clone(),
                    node_id: node.node_id.clone(),
                    child_genotype_id: child.id.clone(),
                    father_genotype_id: father_genotype.map(|item| item.id.clone()),
                    mother_genotype_id: mother_genotype.map(|item| item.id.clone()),
                    uncertain,
                    weight: confidence.max(0.0),
                });
            }
        }
    }

    let block_links = active_links
        .into_iter()
        .filter(|link| {
            let left_key = (link.sample.clone(), link.left_variant.clone());
            let right_key = (link.sample.clone(), link.right_variant.clone());
            node_keys.contains_key(&left_key) && node_keys.contains_key(&right_key)
        })
        .collect::<Vec<_>>();

    let relation_choices = relation_items
        .iter()
        .enumerate()
        .map(|(index, item)| enumerate_relation_choices(input, item, index).unwrap_or_else(|| vec![None]))
        .collect::<Vec<_>>();

    let budget = input.candidate_budget;
    let mut raw_candidates: Vec<RawCandidate> = Vec::new();
    let mut budget_exhausted = false;
    let mut combination = vec![0usize; relation_choices.len()];
    let mut combinations_explored = 0usize;

    loop {
        if combinations_explored >= budget {
            budget_exhausted = true;
            break;
        }
        combinations_explored += 1;
        let mut selected: Vec<Option<Assignment>> = Vec::new();
        for (index, choice_index) in combination.iter().enumerate() {
            selected.push(relation_choices[index].get(*choice_index).cloned().unwrap_or(None));
        }
        let orientations = solve_orientations(input, block, &node_map, &block_links, overlay);
        for mut raw in orientations {
            let mut candidate_score = raw.score;
            let mut assignments = Vec::new();
            for (index, item) in relation_items.iter().enumerate() {
                match &selected[index] {
                    Some(assignment) => {
                        let assignment = assignment.clone();
                        if assignment.ignored {
                            candidate_score -= 0.05;
                        } else {
                            let support_weight = if item.uncertain { 0.05 } else { item.weight };
                            candidate_score += support_weight;
                            candidate_score -= assignment.de_novo_alleles.len() as f64 * 4.0;
                        }
                        assignments.push(assignment);
                    }
                    None => {
                        assignments.push(Assignment {
                            paternal_allele: "*".to_string(),
                            maternal_allele: "*".to_string(),
                            relationship_id: item.relationship_id.clone(),
                            ignored: true,
                            de_novo_alleles: Vec::new(),
                        });
                        candidate_score -= 0.05;
                    }
                }
            }
            assignments.sort();
            raw.assignments = assignments;
            raw.score = candidate_score;
            raw_candidates.push(raw);
            if raw_candidates.len() > budget * 4 {
                raw_candidates.sort_by(|left, right| {
                    right
                        .score
                        .partial_cmp(&left.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(canonical_raw(right, &node_map).cmp(&canonical_raw(left, &node_map)))
                });
                raw_candidates.truncate(budget);
            }
        }

        let mut incremented = false;
        for index in (0..combination.len()).rev() {
            if combination[index] + 1 < relation_choices[index].len() {
                combination[index] += 1;
                for later in index + 1..combination.len() {
                    combination[later] = 0;
                }
                incremented = true;
                break;
            }
        }
        if !incremented {
            break;
        }
        if combinations_explored >= budget {
            budget_exhausted = true;
            break;
        }
        let _ = &mut node_bits;
    }

    if raw_candidates.is_empty() {
        budget_exhausted = true;
    }

    raw_candidates.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(canonical_raw(left, &node_map).cmp(&canonical_raw(right, &node_map)))
    });
    let best_score = raw_candidates.first().map(|item| item.score).unwrap_or(0.0);
    let mut distinct: Vec<RawCandidate> = Vec::new();
    for item in raw_candidates {
        if !distinct
            .iter()
            .any(|existing| canonical_raw(existing, &node_map) == canonical_raw(&item, &node_map))
        {
            distinct.push(item);
        }
    }
    let mut kept_tied = Vec::new();
    let mut dropped_tied = false;
    for item in distinct {
        let tied = (item.score - best_score).abs() < 1e-9;
        if kept_tied.len() < budget || !tied {
            kept_tied.push((item, tied));
        } else {
            dropped_tied = true;
        }
    }
    budget_exhausted |= dropped_tied;

    let candidates = kept_tied
        .iter()
        .enumerate()
        .map(|(ordinal, (raw, tied))| materialize_candidate(input, block, relation_items.as_slice(), raw, *tied, budget_exhausted, ordinal))
        .collect();
    BlockSolution {
        candidates,
        budget_exhausted,
    }
}

fn enumerate_relation_choices(input: &AnalysisInput, item: &RelationItem, index: usize) -> Option<Vec<Option<Assignment>>> {
    let child = input.genotypes.iter().find(|value| value.id == item.child_genotype_id)?;
    let father = item
        .father_genotype_id
        .as_deref()
        .and_then(|id| input.genotypes.iter().find(|value| value.id == id));
    let mother = item
        .mother_genotype_id
        .as_deref()
        .and_then(|id| input.genotypes.iter().find(|value| value.id == id));
    let mut child_alleles = if child.heterozygous() {
        child.alleles.clone()
    } else {
        vec![]
    };
    child_alleles.sort();
    child_alleles.dedup();
    if child_alleles.is_empty() {
        return None;
    }

    let choices = if item.uncertain {
        vec![true, false]
    } else {
        vec![false]
    };
    let mut assignments = Vec::new();
    for ignored in choices {
        if ignored {
            assignments.push(Some(Assignment {
                paternal_allele: "*".to_string(),
                maternal_allele: "*".to_string(),
                relationship_id: item.relationship_id.clone(),
                ignored: true,
                de_novo_alleles: Vec::new(),
            }));
            continue;
        }
        for paternal_allele in &child_alleles {
            let maternal_allele = child_alleles.iter().find(|value| *value != paternal_allele).unwrap_or(paternal_allele);
            let mut de_novo = Vec::new();
            let father_ok = father
                .map(|genotype| {
                    if genotype.can_supply(paternal_allele) {
                        true
                    } else {
                        de_novo.push(paternal_allele.clone());
                        true
                    }
                })
                .unwrap_or(true);
            let mother_ok = mother
                .map(|genotype| {
                    if genotype.can_supply(maternal_allele) {
                        true
                    } else {
                        de_novo.push(maternal_allele.clone());
                        true
                    }
                })
                .unwrap_or(true);
            if father_ok && mother_ok {
                assignments.push(Some(Assignment {
                    paternal_allele: paternal_allele.clone(),
                    maternal_allele: maternal_allele.clone(),
                    relationship_id: item.relationship_id.clone(),
                    ignored: false,
                    de_novo_alleles: de_novo,
                }));
            }
        }
    }
    let _ = index;
    Some(assignments)
}

fn solve_orientations(
    input: &AnalysisInput,
    block: &PhaseBlock,
    node_map: &BTreeMap<String, NodeRef>,
    links: &[&ReadLink],
    overlay: &DecisionOverlay,
) -> Vec<RawCandidate> {
    let ordered_nodes = block
        .nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<Vec<_>>();
    let mut constraints: Vec<(String, String, bool, f64, String)> = Vec::new();
    for link in links {
        let left = block
            .nodes
            .iter()
            .find(|node| node.sample_id == link.sample && node.variant_id == link.left_variant);
        let right = block
            .nodes
            .iter()
            .find(|node| node.sample_id == link.sample && node.variant_id == link.right_variant);
        if let (Some(left), Some(right)) = (left, right) {
            let same_phase = allele_bit(node_map, &left.node_id, &link.left_allele)
                == allele_bit(node_map, &right.node_id, &link.right_allele);
            constraints.push((
                left.node_id.clone(),
                right.node_id.clone(),
                same_phase,
                link.weight,
                link.id.clone(),
            ));
        }
    }

    let mut fixed = BTreeMap::new();
    for node in &block.nodes {
        let key = format!("{}:{}", block.id, node.variant_id);
        if let Some(lock) = overlay.locked_phases.get(&key) {
            let mut locked = [lock.allele_a.clone(), lock.allele_b.clone()];
            locked.sort();
            let mut canonical = node.alleles.clone();
            canonical.sort();
            if locked == canonical {
                let bit = if [lock.allele_a.clone(), lock.allele_b.clone()] == node.alleles {
                    0
                } else {
                    1
                };
                fixed.insert(node.node_id.clone(), bit);
            }
        }
    }

    let mut results = Vec::new();
    let total = 1usize << ordered_nodes.len();
    for mask in 0..total {
        let mut orientation = BTreeMap::new();
        for (index, node_id) in ordered_nodes.iter().enumerate() {
            let bit = ((mask >> index) & 1) as u8;
            if let Some(fixed_bit) = fixed.get(node_id) {
                if bit != *fixed_bit {
                    orientation.clear();
                    break;
                }
            }
            orientation.insert(node_id.clone(), bit);
        }
        if orientation.is_empty() {
            continue;
        }
        let mut feasible = true;
        let mut score = 0.0;
        for (left, right, same_phase, weight, _link_id) in &constraints {
            let satisfied = (*orientation.get(left).unwrap() == *orientation.get(right).unwrap()) == *same_phase;
            if satisfied {
                score += *weight;
            } else {
                score -= weight * 2.0;
                feasible = false;
            }
        }
        for node in &block.nodes {
            if let Some(genotype) = input.genotypes.iter().find(|item| item.id == node.genotype_id) {
                let quality_weight = (genotype.quality.max(0.0).min(100.0) / 100.0)
                    * if genotype.low_quality_heterozygous { 0.5 } else { 1.0 };
                score += quality_weight;
                for key in likelihood_keys(&genotype.alleles) {
                    if let Some(value) = genotype.likelihoods.get(&key) {
                        if *value >= 0.0 && *value <= 1.0 {
                            score += value * 0.01;
                        }
                    }
                }
            }
        }
        if feasible {
            results.push(RawCandidate {
                orientation,
                assignments: Vec::new(),
                score,
            });
        }
    }
    results
}

fn likelihood_keys(alleles: &[String]) -> Vec<String> {
    let mut sorted = alleles.to_vec();
    sorted.sort();
    let value = sorted.join("");
    vec![
        sorted.join("/"),
        sorted.join(","),
        sorted.join("|"),
        value,
    ]
}

fn allele_bit(node_map: &BTreeMap<String, NodeRef>, node_id: &str, allele: &str) -> u8 {
    node_map
        .get(node_id)
        .and_then(|node| node.alleles.iter().position(|value| value == allele))
        .unwrap_or(0) as u8
}

fn canonical_raw(raw: &RawCandidate, node_map: &BTreeMap<String, NodeRef>) -> String {
    let orientation = raw
        .orientation
        .iter()
        .map(|(node_id, bit)| {
            let node = &node_map[node_id];
            let index = *bit as usize;
            format!("{}={}/{}", node.variant_id, node.alleles[index], node.alleles[1 - index])
        })
        .collect::<Vec<_>>()
        .join(";");
    let assignments = raw
        .assignments
        .iter()
        .map(|item| {
            format!(
                "{}:{}:{}:{}:{}",
                item.relationship_id,
                item.paternal_allele,
                item.maternal_allele,
                item.ignored,
                item.de_novo_alleles.join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(";");
    format!("{orientation}|{assignments}")
}

fn materialize_candidate(
    input: &AnalysisInput,
    block: &PhaseBlock,
    relation_items: &[RelationItem],
    raw: &RawCandidate,
    tied: bool,
    budget_limited: bool,
    ordinal: usize,
) -> Candidate {
    let mut orientation = BTreeMap::new();
    let mut support = Vec::new();
    for node in &block.nodes {
        let bit = raw.orientation[&node.node_id] as usize;
        orientation.insert(
            node.variant_id.clone(),
            [node.alleles[bit].clone(), node.alleles[1 - bit].clone()],
        );
        support.push(EvidenceRef {
            observation_id: node.genotype_id.clone(),
            weight: input
                .genotypes
                .iter()
                .find(|item| item.id == node.genotype_id)
                .map(|item| if item.low_quality_heterozygous { item.quality / 100.0 } else { 1.0 })
                .unwrap_or(0.01),
            role: "heterozygous_genotype".to_string(),
        });
    }
    for bridge in &block.bridge_evidence {
        support.push(EvidenceRef {
            observation_id: bridge.observation_id.clone(),
            weight: bridge.weight,
            role: "read_bridge".to_string(),
        });
    }
    for assignment in &raw.assignments {
        if !assignment.ignored {
            let item = relation_items
                .iter()
                .find(|value| value.relationship_id == assignment.relationship_id && {
                    let child = input.genotypes.iter().find(|g| g.id == value.child_genotype_id);
                    child.is_some_and(|child| {
                        let node = block
                            .nodes
                            .iter()
                            .find(|node| node.sample_id == child.sample && node.variant_id == child.variant);
                        node.is_some()
                    })
                })
                .or_else(|| relation_items.iter().find(|value| value.relationship_id == assignment.relationship_id));
            if let Some(item) = item {
                support.push(EvidenceRef {
                    observation_id: item.relationship_id.clone(),
                    weight: if item.uncertain { 0.05 } else { item.weight },
                    role: "mendelian_transmission".to_string(),
                });
                if let Some(id) = &item.father_genotype_id {
                    support.push(EvidenceRef {
                        observation_id: id.clone(),
                        weight: 1.0,
                        role: "paternal_genotype".to_string(),
                    });
                }
                if let Some(id) = &item.mother_genotype_id {
                    support.push(EvidenceRef {
                        observation_id: id.clone(),
                        weight: 1.0,
                        role: "maternal_genotype".to_string(),
                    });
                }
            }
        }
    }

    let relation_sources = raw
        .assignments
        .iter()
        .map(|assignment| {
            let weight = relation_items
                .iter()
                .find(|item| item.relationship_id == assignment.relationship_id)
                .map(|item| if item.uncertain { 0.05 } else { item.weight })
                .unwrap_or(0.0);
            RelationSource {
                relationship_id: assignment.relationship_id.clone(),
                child_variant: relation_items
                    .iter()
                    .find(|item| item.relationship_id == assignment.relationship_id)
                    .and_then(|item| {
                        input
                            .genotypes
                            .iter()
                            .find(|genotype| genotype.id == item.child_genotype_id)
                            .map(|genotype| genotype.variant.clone())
                    })
                    .unwrap_or_default(),
                paternal_allele: assignment.paternal_allele.clone(),
                maternal_allele: assignment.maternal_allele.clone(),
                weight,
                ignored: assignment.ignored,
            }
        })
        .collect();
    let mut de_novo = Vec::new();
    for assignment in &raw.assignments {
        for allele in &assignment.de_novo_alleles {
            let item = relation_items
                .iter()
                .find(|item| item.relationship_id == assignment.relationship_id);
            de_novo.push(DeNovoCall {
                genotype_id: item.map(|item| item.child_genotype_id.clone()).unwrap_or_default(),
                relationship_id: Some(assignment.relationship_id.clone()),
                allele: allele.clone(),
                weight: item.map(|item| if item.uncertain { 0.05 } else { item.weight }).unwrap_or(0.0),
            });
        }
    }

    let mut ignored = Vec::new();
    for assignment in &raw.assignments {
        if assignment.ignored {
            ignored.push(IgnoredData {
                observation_id: assignment.relationship_id.clone(),
                reason: "父母身份不确定；候选忽略该传递约束".to_string(),
            });
        }
    }
    for link in &input.read_links {
        if link.withdrawn || overlay_contains_withdrawn(input, link) {
            let touches_block = block.nodes.iter().any(|node| {
                node.sample_id == link.sample
                    && (node.variant_id == link.left_variant || node.variant_id == link.right_variant)
            });
            if touches_block {
                ignored.push(IgnoredData {
                    observation_id: link.id.clone(),
                    reason: "读段连接已撤回，仅保留为桥接事件".to_string(),
                });
            }
        }
    }
    support.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    ignored.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    ignored.dedup_by(|left, right| left.observation_id == right.observation_id && left.reason == right.reason);
    let signature = format!("{}:{}:{}:{}", input.version_id, block.id, canonical_raw(raw, &node_map_for(block)), ordinal);
    Candidate {
        id: format!("cand-{}-{}", ordinal + 1, stable_hash(&signature)),
        block_id: block.id.clone(),
        score: (raw.score * 1000.0).round() / 1000.0,
        tied,
        orientation,
        relation_sources,
        de_novo,
        support,
        ignored,
        budget_limited,
    }
}

fn node_map_for(block: &PhaseBlock) -> BTreeMap<String, NodeRef> {
    block.nodes.iter().map(|node| (node.node_id.clone(), node.clone())).collect()
}

fn overlay_contains_withdrawn(_input: &AnalysisInput, _link: &ReadLink) -> bool {
    false
}

fn detect_mendel(input: &AnalysisInput, conflicts: &mut Vec<Conflict>) {
    let variants: BTreeMap<&str, &Variant> = input.variants.iter().map(|item| (item.id.as_str(), item)).collect();
    for relationship in &input.relationships {
        for variant in input.variants.iter() {
            let child = input.genotype(&relationship.child, &variant.id);
            let father = relationship
                .father
                .as_deref()
                .and_then(|sample_id| input.genotype(sample_id, &variant.id));
            let mother = relationship
                .mother
                .as_deref()
                .and_then(|sample_id| input.genotype(sample_id, &variant.id));
            let Some(child) = child else {
                continue;
            };
            if child.missing || child.alleles.is_empty() || child.triallelic() {
                continue;
            }
            let mut child_alleles = child.alleles.clone();
            child_alleles.sort();
            child_alleles.dedup();
            let father_genotype = father.filter(|genotype| !genotype.missing && !genotype.alleles.is_empty());
            let mother_genotype = mother.filter(|genotype| !genotype.missing && !genotype.alleles.is_empty());
            if father_genotype.is_none() || mother_genotype.is_none() {
                continue;
            }
            let father_genotype = father_genotype.unwrap();
            let mother_genotype = mother_genotype.unwrap();
            let exact_transmission = child_alleles.iter().any(|paternal| {
                father_genotype.can_supply(paternal)
                    && child_alleles
                        .iter()
                        .any(|maternal| maternal != paternal || child_alleles.len() == 1)
                        && child_alleles
                            .iter()
                            .any(|maternal| (maternal != paternal || child_alleles.len() == 1) && mother_genotype.can_supply(maternal))
            });
            if exact_transmission {
                continue;
            }
            let mut de_novo = child_alleles
                .iter()
                .filter(|allele| {
                    !father_genotype.can_supply(allele)
                        && !mother_genotype.can_supply(allele)
                        && child_alleles
                            .iter()
                            .any(|other| other != *allele && (father_genotype.can_supply(other) || mother_genotype.can_supply(other)))
                })
                .cloned()
                .collect::<Vec<_>>();
            let mut incompatible = if de_novo.is_empty() {
                child_alleles
                    .iter()
                    .filter(|allele| !father_genotype.can_supply(allele) && !mother_genotype.can_supply(allele))
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            if incompatible.is_empty() && de_novo.is_empty() {
                incompatible = child_alleles.clone();
            }
            if !incompatible.is_empty() {
                let mut observations = vec![child.id.clone(), relationship.id.clone()];
                if let Some(genotype) = father {
                    observations.push(genotype.id.clone());
                }
                if let Some(genotype) = mother {
                    observations.push(genotype.id.clone());
                }
                let mut minimal_repairs = vec![RepairSet {
                    observation_ids: observations.clone(),
                    restored_constraint: format!(
                        "隔离等位基因 {:?} 的子代/亲代观测组合后，二倍体传递约束恢复",
                        incompatible
                    ),
                }];
                if let Some(genotype) = father {
                    minimal_repairs.push(RepairSet {
                        observation_ids: vec![genotype.id.clone()],
                        restored_constraint: "忽略父亲基因型后，子代等位基因可由母亲与潜在变异解释".to_string(),
                    });
                }
                if let Some(genotype) = mother {
                    minimal_repairs.push(RepairSet {
                        observation_ids: vec![genotype.id.clone()],
                        restored_constraint: "忽略母亲基因型后，子代等位基因可由父亲与潜在变异解释".to_string(),
                    });
                }
                conflicts.push(Conflict {
                    id: format!(
                        "conflict-mendel-{}-{}",
                        stable_hash(&relationship.id),
                        stable_hash(&variant.id)
                    ),
                    kind: "mendelian_inconsistency".to_string(),
                    severity: if relationship.is_uncertain() {
                        "warning".to_string()
                    } else {
                        "error".to_string()
                    },
                    description: format!(
                        "{} 在 {} 上的等位基因 {:?} 无法由已登记父母共同解释",
                        relationship.child, variant.id, incompatible
                    ),
                    observations,
                    minimal_repairs,
                });
            } else if !de_novo.is_empty() && !relationship.is_uncertain() {
                let observations = vec![
                    child.id.clone(),
                    relationship.id.clone(),
                    father.map(|item| item.id.clone()).unwrap_or_default(),
                    mother.map(|item| item.id.clone()).unwrap_or_default(),
                ]
                .into_iter()
                .filter(|item| !item.is_empty())
                .collect();
                conflicts.push(Conflict {
                    id: format!(
                        "conflict-denovo-{}-{}",
                        stable_hash(&relationship.id),
                        stable_hash(&variant.id)
                    ),
                    kind: "de_novo_candidate".to_string(),
                    severity: "review".to_string(),
                    description: format!(
                        "{} 在 {} 上的 {:?} 更符合 de novo，而不是普通 Mendel 传递",
                        relationship.child, variant.id, de_novo
                    ),
                    observations,
                    minimal_repairs: vec![RepairSet {
                        observation_ids: vec![child.id.clone()],
                        restored_constraint: "将子代等位基因标记为 de novo 后，普通父母传递冲突解除".to_string(),
                    }],
                });
            }
        }
    }
    let _ = variants;
}

fn dedupe_conflicts(items: Vec<Conflict>) -> Vec<Conflict> {
    let mut seen = BTreeSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert((item.id.clone(), item.description.clone())))
        .collect()
}

fn dedupe_warnings(items: Vec<Warning>) -> Vec<Warning> {
    let mut seen = BTreeSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert((item.id.clone(), item.description.clone())))
        .collect()
}
