use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::models::ImportPayload;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub id: String,
    pub label: String,
    pub version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    pub id: String,
    pub chrom: String,
    pub position: i64,
    pub reference: String,
    pub alternate: String,
    pub version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Genotype {
    pub observation_id: String,
    pub sample_id: String,
    pub variant_id: String,
    pub alleles: Vec<String>,
    pub is_missing: bool,
    pub likelihood: f64,
    pub quality: String,
    pub batch_id: String,
    pub version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadLink {
    pub link_id: String,
    pub sample_id: String,
    pub variant_a: String,
    pub variant_b: String,
    pub allele_a_index: usize,
    pub allele_b_index: usize,
    pub weight: f64,
    pub batch_id: String,
    pub version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transmission {
    pub transmission_id: String,
    pub child_id: String,
    pub parent_id: String,
    pub parent_role: String,
    pub variant_id: String,
    pub child_allele_index: usize,
    pub parent_allele_index: usize,
    pub weight: f64,
    pub batch_id: String,
    pub version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub relationship_id: String,
    pub child_id: String,
    pub father_id: Option<String>,
    pub mother_id: Option<String>,
    pub duplicate_of_id: Option<String>,
    pub kind: String,
    pub confidence: String,
    pub batch_id: String,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionInfo {
    pub version: i64,
    pub batch_id: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct IgnoredObservation {
    pub observation_id: String,
    pub kind: String,
    pub reason: String,
    pub weight: f64,
    pub candidate_specific: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceRef {
    pub observation_id: String,
    pub kind: String,
    pub weight: f64,
    pub satisfied: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeView {
    pub sample_id: String,
    pub variant_id: String,
    pub allele0: String,
    pub allele1: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct CandidateView {
    pub candidate_id: String,
    pub signature: String,
    pub score: f64,
    pub orientation: BTreeMap<String, u8>,
    pub supports: Vec<EvidenceRef>,
    pub ignored_observations: Vec<IgnoredObservation>,
    pub label_swap_equivalent: bool,
    pub accepted: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct BridgeEvidence {
    pub bridge_id: String,
    pub observation_id: String,
    pub kind: String,
    pub left_block: String,
    pub right_block: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct BlockView {
    pub block_id: String,
    pub chrom: String,
    pub nodes: Vec<NodeView>,
    pub candidates: Vec<CandidateView>,
    pub bridge_evidence: Vec<BridgeEvidence>,
    pub budget_reached: bool,
    pub search_incomplete: bool,
    pub no_unique_solution: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct ConflictView {
    pub conflict_id: String,
    pub kind: String,
    pub severity: String,
    pub chrom: String,
    pub position: i64,
    pub variant_id: String,
    pub relationship_id: String,
    pub observations: Vec<String>,
    pub minimal_repair_sets: Vec<Vec<String>>,
    pub explanation: String,
    pub resolved: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct DecisionView {
    pub event_id: i64,
    pub kind: String,
    pub payload: Value,
    pub input_version: Option<i64>,
    pub effective: bool,
    pub stale: bool,
    pub stale_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReplayStep {
    pub event_id: i64,
    pub kind: String,
    pub payload: Value,
    pub active: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct State {
    pub title: String,
    pub branch: String,
    pub versions: Vec<VersionInfo>,
    pub pinned_version: Option<i64>,
    pub effective_version: i64,
    pub samples: Vec<Sample>,
    pub variants: Vec<Variant>,
    pub relationships: Vec<Relationship>,
    pub genotypes: Vec<Genotype>,
    pub read_links: Vec<ReadLink>,
    pub transmissions: Vec<Transmission>,
    pub blocks: Vec<BlockView>,
    pub conflicts: Vec<ConflictView>,
    pub ignored_observations: Vec<IgnoredObservation>,
    pub decisions: Vec<DecisionView>,
    pub withdrawn_read_links: Vec<String>,
    pub locks: BTreeMap<String, String>,
    pub budget: usize,
    pub replay_steps: Vec<ReplayStep>,
}

#[derive(Debug, Clone)]
struct RawState {
    samples: BTreeMap<String, Sample>,
    variants: BTreeMap<String, Variant>,
    relationships: BTreeMap<String, Relationship>,
    genotypes: BTreeMap<String, Genotype>,
    read_links: BTreeMap<String, ReadLink>,
    transmissions: BTreeMap<String, Transmission>,
}

#[derive(Debug, Clone)]
struct EventRec {
    id: i64,
    kind: String,
    payload: Value,
    input_version: Option<i64>,
}

#[derive(Debug, Clone)]
struct Adjudication {
    pinned: Option<i64>,
    accepted: BTreeMap<String, (String, i64, Option<i64>)>,
    locks: BTreeMap<String, (String, i64, Option<i64>)>,
    withdrawn: BTreeSet<String>,
    relation_overrides: BTreeMap<String, (Value, i64, Option<i64>)>,
    uncertain_relations: BTreeSet<String>,
    accepted_denovo: BTreeSet<String>,
    decisions: Vec<(EventRec, bool, Option<String>)>,
}

#[derive(Debug, Clone)]
struct GraphNode {
    key: String,
    sample: String,
    variant: String,
    alleles: Vec<String>,
}
#[derive(Debug, Clone)]
struct XorEdge {
    id: String,
    kind: String,
    left: String,
    right: String,
    parity: u8,
    weight: f64,
}
#[derive(Debug, Clone)]
struct UnaryConstraint {
    id: String,
    kind: String,
    node: String,
    bit: u8,
    weight: f64,
}
#[derive(Debug, Clone)]
struct PhysicalEdge {
    id: String,
    kind: String,
    left: String,
    right: String,
}

#[derive(Debug, Clone)]
struct Candidate {
    assignment: BTreeMap<String, u8>,
    score: f64,
    satisfied: BTreeSet<String>,
    ignored: BTreeMap<String, IgnoredObservation>,
}

pub fn build_state(conn: &Connection, branch: &str, budget: usize) -> Result<State, String> {
    let versions = load_versions(conn)?;
    let events = branch_events(conn, branch)?;
    let mut adjudication = Adjudication {
        pinned: None,
        accepted: BTreeMap::new(),
        locks: BTreeMap::new(),
        withdrawn: BTreeSet::new(),
        relation_overrides: BTreeMap::new(),
        uncertain_relations: BTreeSet::new(),
        accepted_denovo: BTreeSet::new(),
        decisions: Vec::new(),
    };
    let rollbacks: HashSet<i64> = events
        .iter()
        .filter(|event| event.kind == "rollback_decision")
        .filter_map(|event| event.payload.get("event_id").and_then(Value::as_i64))
        .collect();
    let mut replay_steps = Vec::new();

    for event in &events {
        let active = event.kind != "rollback_decision" && !rollbacks.contains(&event.id);
        replay_steps.push(ReplayStep {
            event_id: event.id,
            kind: event.kind.clone(),
            payload: event.payload.clone(),
            active,
        });
        apply_event(&mut adjudication, event, active);
    }

    let pinned = adjudication.pinned;
    let mut raw = RawState {
        samples: BTreeMap::new(),
        variants: BTreeMap::new(),
        relationships: BTreeMap::new(),
        genotypes: BTreeMap::new(),
        read_links: BTreeMap::new(),
        transmissions: BTreeMap::new(),
    };
    let max_version = versions.last().map(|v| v.0).unwrap_or(0);
    let effective = pinned.unwrap_or(max_version);
    for (version, payload) in load_payloads(conn)? {
        if version > effective {
            break;
        }
        merge_payload(&mut raw, version, payload);
    }
    apply_relation_overrides(&mut raw, &adjudication);

    let mut state = State {
        title: "相位织图".into(),
        branch: branch.into(),
        versions: versions
            .iter()
            .map(|(v, b)| VersionInfo {
                version: *v,
                batch_id: b.clone(),
                pinned: pinned == Some(*v),
            })
            .collect(),
        pinned_version: pinned,
        effective_version: effective,
        samples: raw.samples.values().cloned().collect(),
        variants: raw.variants.values().cloned().collect(),
        relationships: raw.relationships.values().cloned().collect(),
        genotypes: raw.genotypes.values().cloned().collect(),
        read_links: raw.read_links.values().cloned().collect(),
        transmissions: raw.transmissions.values().cloned().collect(),
        blocks: Vec::new(),
        conflicts: Vec::new(),
        ignored_observations: Vec::new(),
        decisions: Vec::new(),
        withdrawn_read_links: adjudication.withdrawn.iter().cloned().collect(),
        locks: adjudication
            .locks
            .iter()
            .map(|(k, v)| (k.clone(), v.0.clone()))
            .collect(),
        budget,
        replay_steps,
    };

    let analysis = analyze(&raw, &adjudication, budget)?;
    state.ignored_observations = analysis.ignored.clone();
    state.blocks = analysis.blocks.clone();
    state.conflicts = analysis.conflicts.clone();
    state.decisions = finalize_decisions(&adjudication, &state, &analysis);
    Ok(state)
}

fn load_versions(conn: &Connection) -> Result<Vec<(i64, String)>, String> {
    let mut stmt = conn
        .prepare("select version, batch_id from imports order by version")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn load_payloads(conn: &Connection) -> Result<Vec<(i64, ImportPayload)>, String> {
    let mut stmt = conn
        .prepare("select version, payload from imports order by version")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        let (version, raw) = row.map_err(|e| e.to_string())?;
        let payload = serde_json::from_str::<ImportPayload>(&raw).map_err(|e| e.to_string())?;
        out.push((version, payload));
    }
    Ok(out)
}

fn branch_events(conn: &Connection, branch: &str) -> Result<Vec<EventRec>, String> {
    let mut pending = vec![(branch.to_string(), None::<i64>)];
    let mut gathered: BTreeMap<i64, EventRec> = BTreeMap::new();
    while let Some((name, cutoff)) = pending.pop() {
        let row = conn
            .query_row(
                "select parent_branch, fork_event_id from branches where name=?1",
                rusqlite::params![name],
                |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<i64>>(1)?)),
            )
            .map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("select event_id, kind, payload, input_version from events where branch=?1 order by event_id")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![name], |r| {
                Ok(EventRec {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    payload: serde_json::from_str(&r.get::<_, String>(2)?).unwrap_or(Value::Null),
                    input_version: r.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for item in rows {
            let event = item.map_err(|e| e.to_string())?;
            if cutoff.is_none_or(|cut| event.id <= cut) && !gathered.contains_key(&event.id) {
                gathered.insert(event.id, event);
            }
        }
        if let Some(parent) = row.0 {
            pending.push((parent, row.1));
        }
    }
    Ok(gathered.into_values().collect())
}

fn apply_event(decisions: &mut Adjudication, event: &EventRec, active: bool) {
    if active {
        match event.kind.as_str() {
            "pin_version" => {
                if let Some(version) = event.payload.get("version").and_then(Value::as_i64) {
                    decisions.pinned = Some(version);
                }
            }
            "accept_candidate" => {
                if let (Some(block), Some(signature)) = (
                    str_field(event, "block_id"),
                    str_field(event, "candidate_signature"),
                ) {
                    decisions
                        .accepted
                        .insert(block, (signature, event.id, event.input_version));
                }
            }
            "lock_phase" => {
                if let (Some(sample), Some(variant), Some(origin)) = (
                    str_field(event, "sample_id"),
                    str_field(event, "variant_id"),
                    str_field(event, "haplotype_a_origin"),
                ) {
                    decisions.locks.insert(
                        format!("{sample}:{variant}"),
                        (origin, event.id, event.input_version),
                    );
                }
            }
            "withdraw_read_link" => {
                if let Some(link) = str_field(event, "read_link_id") {
                    decisions.withdrawn.insert(link);
                }
            }
            "revise_relationship" => {
                if let Some(rel) = str_field(event, "relationship_id") {
                    decisions
                        .relation_overrides
                        .insert(rel, (event.payload.clone(), event.id, event.input_version));
                }
            }
            "mark_relationship_uncertain" => {
                if let Some(rel) = str_field(event, "relationship_id") {
                    decisions.uncertain_relations.insert(rel);
                }
            }
            "accept_de_novo" => {
                if let Some(conflict) = str_field(event, "conflict_id") {
                    decisions.accepted_denovo.insert(conflict);
                }
            }
            _ => {}
        }
    }
    decisions.decisions.push((event.clone(), active, None));
}

fn str_field(event: &EventRec, key: &str) -> Option<String> {
    event
        .payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn merge_payload(raw: &mut RawState, version: i64, payload: ImportPayload) {
    for item in payload.samples {
        raw.samples.insert(
            item.anon_id.clone(),
            Sample {
                id: item.anon_id,
                label: item.label,
                version,
            },
        );
    }
    for item in payload.variants {
        raw.variants.insert(
            item.variant_id.clone(),
            Variant {
                id: item.variant_id,
                chrom: item.chrom,
                position: item.position,
                reference: item.reference,
                alternate: item.alternate,
                version,
            },
        );
    }
    for item in payload.relationships {
        let id = item.relationship_id.clone().unwrap_or_else(|| {
            format!(
                "rel:{}:{}:{}:{}",
                item.child_id,
                item.father_id.as_deref().unwrap_or("?"),
                item.mother_id.as_deref().unwrap_or("?"),
                item.duplicate_of_id.as_deref().unwrap_or("?")
            )
        });
        raw.relationships.insert(
            id.clone(),
            Relationship {
                relationship_id: id,
                child_id: item.child_id,
                father_id: item.father_id,
                mother_id: item.mother_id,
                duplicate_of_id: item.duplicate_of_id,
                kind: item.kind,
                confidence: item.confidence,
                batch_id: item.source_batch,
                version,
            },
        );
    }
    for item in payload.genotypes {
        let id = item
            .observation_id
            .clone()
            .unwrap_or_else(|| format!("obs:{}:{}", item.sample_id, item.variant_id));
        raw.genotypes.insert(
            id.clone(),
            Genotype {
                observation_id: id,
                sample_id: item.sample_id,
                variant_id: item.variant_id,
                alleles: item.alleles,
                is_missing: item.is_missing,
                likelihood: item.likelihood,
                quality: item.quality,
                batch_id: item.batch_id,
                version,
            },
        );
    }
    for item in payload.read_links {
        raw.read_links.insert(
            item.link_id.clone(),
            ReadLink {
                link_id: item.link_id,
                sample_id: item.sample_id,
                variant_a: item.variant_a,
                variant_b: item.variant_b,
                allele_a_index: item.allele_a_index,
                allele_b_index: item.allele_b_index,
                weight: item.weight,
                batch_id: item.batch_id,
                version,
            },
        );
    }
    for item in payload.transmissions {
        raw.transmissions.insert(
            item.transmission_id.clone(),
            Transmission {
                transmission_id: item.transmission_id,
                child_id: item.child_id,
                parent_id: item.parent_id,
                parent_role: item.parent_role,
                variant_id: item.variant_id,
                child_allele_index: item.child_allele_index,
                parent_allele_index: item.parent_allele_index,
                weight: item.weight,
                batch_id: item.batch_id,
                version,
            },
        );
    }
}

fn apply_relation_overrides(raw: &mut RawState, adjudication: &Adjudication) {
    for (id, (payload, _, _)) in &adjudication.relation_overrides {
        if let Some(existing) = raw.relationships.get_mut(id) {
            if let Some(value) = payload.get("father_id") {
                existing.father_id = value.as_str().map(str::to_string);
            }
            if let Some(value) = payload.get("mother_id") {
                existing.mother_id = value.as_str().map(str::to_string);
            }
            if let Some(value) = payload.get("duplicate_of_id") {
                existing.duplicate_of_id = value.as_str().map(str::to_string);
            }
            if let Some(value) = payload.get("confidence").and_then(Value::as_str) {
                existing.confidence = value.into();
            }
            if let Some(value) = payload.get("note").and_then(Value::as_str) {
                existing.batch_id = format!("decision:{}", value);
            }
            existing.kind = "revised".into();
        }
    }
    for id in &adjudication.uncertain_relations {
        if let Some(existing) = raw.relationships.get_mut(id) {
            existing.confidence = "uncertain".into();
        }
    }
}

struct Analysis {
    blocks: Vec<BlockView>,
    conflicts: Vec<ConflictView>,
    ignored: Vec<IgnoredObservation>,
}

fn analyze(raw: &RawState, decisions: &Adjudication, budget: usize) -> Result<Analysis, String> {
    let mut ignored_map: BTreeMap<String, IgnoredObservation> = BTreeMap::new();
    let ignore = |map: &mut BTreeMap<String, IgnoredObservation>,
                  id: &str,
                  kind: &str,
                  reason: &str,
                  weight: f64,
                  candidate_specific: bool| {
        map.entry(id.to_string())
            .or_insert_with(|| IgnoredObservation {
                observation_id: id.into(),
                kind: kind.into(),
                reason: reason.into(),
                weight,
                candidate_specific,
            });
    };

    let mut nodes: BTreeMap<String, GraphNode> = BTreeMap::new();
    for genotype in raw.genotypes.values() {
        if let Some(variant) = raw.variants.get(&genotype.variant_id) {
            if genotype.is_missing {
                ignore(
                    &mut ignored_map,
                    &genotype.observation_id,
                    "missing_genotype",
                    "基因型缺失，不能成为相位节点或硬传递约束",
                    0.0,
                    false,
                );
            } else if genotype.alleles.len() != 2 {
                ignore(
                    &mut ignored_map,
                    &genotype.observation_id,
                    "invalid_ploidy",
                    "观测不是二倍体双等位记录",
                    genotype.likelihood,
                    false,
                );
            } else {
                let mut unique: BTreeSet<&String> = genotype.alleles.iter().collect();
                unique.remove(&variant.reference);
                unique.remove(&variant.alternate);
                if !genotype.quality.eq_ignore_ascii_case("high") {
                    ignore(
                        &mut ignored_map,
                        &genotype.observation_id,
                        "low_quality_heterozygous",
                        "低质量杂合调用不用于相位节点",
                        genotype.likelihood,
                        false,
                    );
                } else if !unique.is_empty() {
                    ignore(
                        &mut ignored_map,
                        &genotype.observation_id,
                        "triallelic_call",
                        "出现参考/备选之外的第三等位",
                        genotype.likelihood,
                        false,
                    );
                } else if genotype.alleles[0] != genotype.alleles[1] {
                    let key = node_key(&genotype.sample_id, &genotype.variant_id);
                    nodes.insert(
                        key.clone(),
                        GraphNode {
                            key,
                            sample: genotype.sample_id.clone(),
                            variant: genotype.variant_id.clone(),
                            alleles: vec![variant.reference.clone(), variant.alternate.clone()],
                        },
                    );
                }
            }
        } else {
            ignore(
                &mut ignored_map,
                &genotype.observation_id,
                "unknown_variant",
                "变异未在当前输入版本定义",
                genotype.likelihood,
                false,
            );
        }
    }

    let mut xor: Vec<XorEdge> = Vec::new();
    let mut unary: Vec<UnaryConstraint> = Vec::new();
    let mut physical: Vec<PhysicalEdge> = Vec::new();

    for link in raw.read_links.values() {
        if decisions.withdrawn.contains(&link.link_id) {
            continue;
        }
        let left = node_key(&link.sample_id, &link.variant_a);
        let right = node_key(&link.sample_id, &link.variant_b);
        if !nodes.contains_key(&left) || !nodes.contains_key(&right) {
            ignore(
                &mut ignored_map,
                &link.link_id,
                "read_link_unusable",
                "连接的至少一个节点缺失、低质量或非双等位杂合",
                link.weight,
                false,
            );
            continue;
        }
        if nodes.get(&left).unwrap().variant == nodes.get(&right).unwrap().variant {
            ignore(
                &mut ignored_map,
                &link.link_id,
                "read_link_same_variant",
                "读段连接必须连接两个变异位置",
                link.weight,
                false,
            );
            continue;
        }
        let same_chrom = raw
            .variants
            .get(&link.variant_a)
            .zip(raw.variants.get(&link.variant_b))
            .map(|(left_variant, right_variant)| left_variant.chrom == right_variant.chrom)
            .unwrap_or(false);
        if !same_chrom {
            ignore(
                &mut ignored_map,
                &link.link_id,
                "read_link_cross_chromosome",
                "跨染色体读段不能合并 phase block",
                link.weight,
                false,
            );
            continue;
        }
        if link.allele_a_index > 1 || link.allele_b_index > 1 {
            ignore(
                &mut ignored_map,
                &link.link_id,
                "read_link_bad_allele_index",
                "等位索引超出双等位范围",
                link.weight,
                false,
            );
            continue;
        }
        let parity = (link.allele_a_index != link.allele_b_index) as u8;
        xor.push(XorEdge {
            id: link.link_id.clone(),
            kind: "read_link".into(),
            left: left.clone(),
            right: right.clone(),
            parity,
            weight: link.weight,
        });
        physical.push(PhysicalEdge {
            id: link.link_id.clone(),
            kind: "read_link".into(),
            left,
            right,
        });
    }

    add_transmission_constraints(
        raw,
        &nodes,
        &mut xor,
        &mut unary,
        &mut physical,
        &mut ignored_map,
        &ignore,
    );

    let conflicts = build_conflicts(raw, decisions, &mut ignored_map, &ignore);
    let blocks = build_blocks(
        raw,
        &nodes,
        &xor,
        &unary,
        &physical,
        decisions,
        &mut ignored_map,
        &ignore,
        budget,
    );
    let ignored = ignored_map.into_values().collect();
    Ok(Analysis {
        blocks,
        conflicts,
        ignored,
    })
}

fn node_key(sample: &str, variant: &str) -> String {
    format!("{sample}:{variant}")
}

fn add_transmission_constraints(
    raw: &RawState,
    nodes: &BTreeMap<String, GraphNode>,
    xor: &mut Vec<XorEdge>,
    unary: &mut Vec<UnaryConstraint>,
    physical: &mut Vec<PhysicalEdge>,
    ignored: &mut BTreeMap<String, IgnoredObservation>,
    ignore: &impl Fn(&mut BTreeMap<String, IgnoredObservation>, &str, &str, &str, f64, bool),
) {
    for tx in raw.transmissions.values() {
        let child_node_id = node_key(&tx.child_id, &tx.variant_id);
        let child = match nodes.get(&child_node_id) {
            Some(v) => v,
            None => {
                ignore(
                    ignored,
                    &tx.transmission_id,
                    "transmission_unusable",
                    "子节点缺失、低质量或非双等位杂合",
                    tx.weight,
                    false,
                );
                continue;
            }
        };
        if tx.child_allele_index > 1 || tx.parent_allele_index > 1 {
            ignore(
                ignored,
                &tx.transmission_id,
                "transmission_bad_allele_index",
                "等位索引超出双等位范围",
                tx.weight,
                false,
            );
            continue;
        }
        let role_bit = role_bit(&tx.parent_role);
        let child_bit = child
            .alleles
            .iter()
            .position(|a| a == &child.alleles[tx.child_allele_index])
            .unwrap_or(tx.child_allele_index) as u8;
        if let Some(bit) = role_bit {
            let expected = if bit == 0 { child_bit } else { 1 - child_bit };
            unary.push(UnaryConstraint {
                id: tx.transmission_id.clone(),
                kind: format!("transmission:{}", tx.parent_role),
                node: child_node_id.clone(),
                bit: expected,
                weight: 2.0 + tx.weight,
            });
            let parent_node_id = node_key(&tx.parent_id, &tx.variant_id);
            if let Some(parent) = nodes.get(&parent_node_id) {
                if parent.variant != child.variant {
                    ignore(
                        ignored,
                        &tx.transmission_id,
                        "transmission_cross_variant",
                        "传递只能连接同一变异位置的亲子节点",
                        tx.weight,
                        false,
                    );
                    continue;
                }
                let same_chrom = raw
                    .variants
                    .get(&child.variant)
                    .zip(raw.variants.get(&parent.variant))
                    .map(|(a, b)| a.chrom == b.chrom)
                    .unwrap_or(false);
                if !same_chrom {
                    ignore(
                        ignored,
                        &tx.transmission_id,
                        "transmission_cross_chromosome",
                        "跨染色体传递不能合并 phase block",
                        tx.weight,
                        false,
                    );
                    continue;
                }
                let parent_bit = parent
                    .alleles
                    .iter()
                    .position(|a| a == &parent.alleles[tx.parent_allele_index])
                    .unwrap_or(tx.parent_allele_index) as u8;
                let parity = child_bit ^ parent_bit;
                xor.push(XorEdge {
                    id: format!("xor:{}", tx.transmission_id),
                    kind: "transmission_link".into(),
                    left: child_node_id.clone(),
                    right: parent_node_id.clone(),
                    parity,
                    weight: 1.0 + tx.weight,
                });
                physical.push(PhysicalEdge {
                    id: format!("physical:{}", tx.transmission_id),
                    kind: "transmission".into(),
                    left: child_node_id,
                    right: parent_node_id.clone(),
                });
            }
        } else {
            ignore(
                ignored,
                &tx.transmission_id,
                "transmission_unknown_role",
                "父母身份未确定，传递证据保留但不锚定来源",
                tx.weight,
                false,
            );
        }
    }

    for rel in raw
        .relationships
        .values()
        .filter(|r| r.confidence != "uncertain" && (r.father_id.is_some() || r.mother_id.is_some()))
    {
        add_homozygous_parent_anchors(raw, rel, nodes, unary);
    }
}

fn role_bit(role: &str) -> Option<u8> {
    let value = role.to_ascii_lowercase();
    if value.contains("father") || value.contains("父") {
        Some(0)
    } else if value.contains("mother") || value.contains("母") {
        Some(1)
    } else {
        None
    }
}

fn add_homozygous_parent_anchors(
    raw: &RawState,
    rel: &Relationship,
    nodes: &BTreeMap<String, GraphNode>,
    unary: &mut Vec<UnaryConstraint>,
) {
    let Some(child_geno) = raw.genotypes.values().find(|g| {
        g.sample_id == rel.child_id && !g.is_missing && g.alleles.len() == 2 && g.quality == "high"
    }) else {
        return;
    };
    let Some(variant) = raw.variants.get(&child_geno.variant_id) else {
        return;
    };
    let child_node_id = node_key(&rel.child_id, &variant.id);
    if !nodes.contains_key(&child_node_id) {
        return;
    }
    let child_set: BTreeSet<&String> = child_geno.alleles.iter().collect();
    if let Some(father) = &rel.father_id {
        if let Some(parent) = raw.genotypes.values().find(|g| {
            g.sample_id == *father
                && g.variant_id == variant.id
                && !g.is_missing
                && g.alleles.len() == 2
                && g.quality == "high"
        }) {
            if parent.alleles[0] == parent.alleles[1] && child_set.contains(&parent.alleles[0]) {
                let bit = if parent.alleles[0] == variant.reference {
                    0
                } else {
                    1
                };
                let id = format!(
                    "derived-anchor:{}:father:{}",
                    rel.relationship_id, variant.id
                );
                if !unary
                    .iter()
                    .any(|u| u.node == child_node_id && u.bit == bit)
                {
                    unary.push(UnaryConstraint {
                        id,
                        kind: "derived_homozygous_father".into(),
                        node: child_node_id.clone(),
                        bit,
                        weight: 2.0 + parent.likelihood,
                    });
                }
            }
        }
    }
    if let Some(mother) = &rel.mother_id {
        if let Some(parent) = raw.genotypes.values().find(|g| {
            g.sample_id == *mother
                && g.variant_id == variant.id
                && !g.is_missing
                && g.alleles.len() == 2
                && g.quality == "high"
        }) {
            if parent.alleles[0] == parent.alleles[1] && child_set.contains(&parent.alleles[0]) {
                let parent_bit = if parent.alleles[0] == variant.reference {
                    0
                } else {
                    1
                };
                let child_bit = 1 - parent_bit;
                let id = format!(
                    "derived-anchor:{}:mother:{}",
                    rel.relationship_id, variant.id
                );
                if !unary
                    .iter()
                    .any(|u| u.node == child_node_id && u.bit == child_bit)
                {
                    unary.push(UnaryConstraint {
                        id,
                        kind: "derived_homozygous_mother".into(),
                        node: child_node_id,
                        bit: child_bit,
                        weight: 2.0 + parent.likelihood,
                    });
                }
            }
        }
    }
}

fn build_conflicts(
    raw: &RawState,
    decisions: &Adjudication,
    ignored: &mut BTreeMap<String, IgnoredObservation>,
    ignore: &impl Fn(&mut BTreeMap<String, IgnoredObservation>, &str, &str, &str, f64, bool),
) -> Vec<ConflictView> {
    let mut conflicts = Vec::new();
    for rel in raw.relationships.values() {
        if rel.confidence == "uncertain" {
            conflicts.push(ConflictView {
                conflict_id: stable_id(&format!("uncertain:{}", rel.relationship_id)),
                kind: "relationship_identity_uncertain".into(),
                severity: "review".into(),
                chrom: String::new(),
                position: 0,
                variant_id: String::new(),
                relationship_id: rel.relationship_id.clone(),
                observations: Vec::new(),
                minimal_repair_sets: Vec::new(),
                explanation: "父母身份或样本关系待确认；关系不参与硬 Mendelian 裁定。".into(),
                resolved: false,
            });
            continue;
        }
        if let Some(duplicate_of) = &rel.duplicate_of_id {
            for variant in raw.variants.values() {
                if let (Some(left), Some(right)) = (
                    raw.genotypes
                        .values()
                        .find(|g| g.sample_id == rel.child_id && g.variant_id == variant.id),
                    raw.genotypes
                        .values()
                        .find(|g| g.sample_id == *duplicate_of && g.variant_id == variant.id),
                ) {
                    if let Some(conflict) = duplicate_conflict(raw, rel, variant, left, right) {
                        conflicts.push(conflict);
                    }
                }
            }
            continue;
        }
        if rel.father_id.is_none() && rel.mother_id.is_none() {
            continue;
        }
        for variant in raw.variants.values() {
            let child = raw
                .genotypes
                .values()
                .find(|g| g.sample_id == rel.child_id && g.variant_id == variant.id);
            let father = rel.father_id.as_ref().and_then(|id| {
                raw.genotypes
                    .values()
                    .find(|g| g.sample_id == *id && g.variant_id == variant.id)
            });
            let mother = rel.mother_id.as_ref().and_then(|id| {
                raw.genotypes
                    .values()
                    .find(|g| g.sample_id == *id && g.variant_id == variant.id)
            });
            if let Some(conflict) =
                trio_conflict(raw, rel, variant, child, father, mother, ignored, ignore)
            {
                conflicts.push(conflict);
            }
        }
    }

    let hard: Vec<&ConflictView> = conflicts.iter().filter(|c| c.severity == "hard").collect();
    if hard.len() > 1 {
        let global = minimum_hitting_sets(
            &hard
                .iter()
                .map(|c| c.minimal_repair_sets.clone())
                .collect::<Vec<_>>(),
            300,
        );
        for conflict in conflicts.iter_mut().filter(|c| c.severity == "hard") {
            conflict.minimal_repair_sets = global.clone();
        }
    }
    for conflict in &mut conflicts {
        conflict.resolved = decisions.accepted_denovo.contains(&conflict.conflict_id);
    }
    conflicts
}

fn usable_biallelic(g: &Genotype, variant: &Variant) -> Option<Vec<String>> {
    if g.is_missing || g.quality != "high" || g.alleles.len() != 2 {
        return None;
    }
    if g.alleles
        .iter()
        .all(|a| a == &variant.reference || a == &variant.alternate)
    {
        Some(g.alleles.clone())
    } else {
        None
    }
}

fn triallelic(g: &Genotype, variant: &Variant) -> bool {
    !g.is_missing
        && g.alleles.len() == 2
        && g.alleles
            .iter()
            .any(|a| a != &variant.reference && a != &variant.alternate)
}

fn duplicate_conflict(
    raw: &RawState,
    rel: &Relationship,
    variant: &Variant,
    left: &Genotype,
    right: &Genotype,
) -> Option<ConflictView> {
    if left.is_missing || right.is_missing {
        return Some(info_conflict(
            rel,
            variant,
            "missing_duplicate_observation",
            "重复样本之一缺失，无法核对一致调用。",
            vec![left.observation_id.clone(), right.observation_id.clone()],
        ));
    }
    if left.quality != "high" || right.quality != "high" {
        return Some(info_conflict(
            rel,
            variant,
            "low_quality_duplicate_observation",
            "重复样本存在低质量调用，已隔离但不作为硬冲突。",
            vec![left.observation_id.clone(), right.observation_id.clone()],
        ));
    }
    if triallelic(left, variant) || triallelic(right, variant) {
        let observations = vec![left.observation_id.clone(), right.observation_id.clone()];
        return Some(make_hard(
            rel,
            variant,
            "triallelic_error",
            observations,
            vec![
                vec![left.observation_id.clone()],
                vec![right.observation_id.clone()],
            ],
            "第三等位不能来自重复样本一致调用。",
        ));
    }
    let left_set: BTreeSet<&String> = left.alleles.iter().collect();
    let right_set: BTreeSet<&String> = right.alleles.iter().collect();
    if left.alleles.len() == 2 && right.alleles.len() == 2 && left_set != right_set {
        let observations = vec![left.observation_id.clone(), right.observation_id.clone()];
        Some(make_hard(
            rel,
            variant,
            "duplicate_mismatch",
            observations,
            vec![
                vec![left.observation_id.clone()],
                vec![right.observation_id.clone()],
            ],
            "重复样本的双等位基因型不一致。",
        ))
    } else {
        let _ = raw;
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn trio_conflict(
    _raw: &RawState,
    rel: &Relationship,
    variant: &Variant,
    child: Option<&Genotype>,
    father: Option<&Genotype>,
    mother: Option<&Genotype>,
    ignored: &mut BTreeMap<String, IgnoredObservation>,
    ignore: &impl Fn(&mut BTreeMap<String, IgnoredObservation>, &str, &str, &str, f64, bool),
) -> Option<ConflictView> {
    let observations = [child, father, mother]
        .into_iter()
        .flatten()
        .map(|g| g.observation_id.clone())
        .collect::<Vec<_>>();
    let missing = [
        &rel.child_id,
        rel.father_id.as_deref().unwrap_or(""),
        rel.mother_id.as_deref().unwrap_or(""),
    ]
    .iter()
    .zip([child, father, mother])
    .filter_map(|(sample, g)| {
        if g.is_none() && !sample.is_empty() {
            Some((*sample).to_string())
        } else {
            None
        }
    })
    .collect::<Vec<_>>();
    if !missing.is_empty() {
        for g in [child, father, mother].into_iter().flatten() {
            ignore(
                ignored,
                &g.observation_id,
                "relationship_incomplete",
                "家系观测不完整，不能形成硬传递判断",
                g.likelihood,
                false,
            );
        }
        return Some(info_conflict(
            rel,
            variant,
            "missing_genotype",
            &format!("缺失成员 {} 的观测，证据隔离而非删行。", missing.join(", ")),
            observations,
        ));
    }
    let (child, father, mother) = (child.unwrap(), father, mother);
    let present = [Some(child), father, mother]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if present.iter().any(|g| g.is_missing) {
        for g in &present {
            ignore(
                ignored,
                &g.observation_id,
                "missing_genotype",
                "缺失基因型隔离为证据，不参与硬 Mendelian 判断",
                g.likelihood,
                false,
            );
        }
        return Some(info_conflict(
            rel,
            variant,
            "missing_genotype",
            "存在缺失基因型；证据保留且不能当作可删除行。",
            observations,
        ));
    }
    for g in present {
        if g.quality != "high" {
            ignore(
                ignored,
                &g.observation_id,
                "low_quality_family_call",
                "低质量家系调用不用于 Mendelian 约束",
                g.likelihood,
                false,
            );
            return Some(info_conflict(
                rel,
                variant,
                "low_quality_heterozygous",
                "低质量调用被隔离，不能触发删除或硬不一致。",
                vec![g.observation_id.clone()],
            ));
        }
    }
    if triallelic(child, variant)
        || father.is_some_and(|g| triallelic(g, variant))
        || mother.is_some_and(|g| triallelic(g, variant))
    {
        let bad = [Some(child), father, mother]
            .into_iter()
            .flatten()
            .filter(|g| triallelic(g, variant))
            .map(|g| g.observation_id.clone())
            .collect::<Vec<_>>();
        let repairs = bad.iter().map(|id| vec![id.clone()]).collect();
        return Some(make_hard(
            rel,
            variant,
            "triallelic_error",
            observations,
            repairs,
            "第三等位视为候选测序错误；移除任一错误观测即可恢复双等位约束。",
        ));
    }
    let child_alleles = usable_biallelic(child, variant)?;
    let father_alleles = father.map(|g| usable_biallelic(g, variant)).unwrap_or(None);
    let mother_alleles = mother.map(|g| usable_biallelic(g, variant)).unwrap_or(None);
    let repairs = mendelian_repairs(
        &child_alleles,
        father_alleles.as_ref(),
        mother_alleles.as_ref(),
        child,
        father,
        mother,
    );
    if repairs.is_empty() {
        None
    } else {
        let denovo = child_alleles.iter().any(|a| {
            father_alleles.as_ref().is_none_or(|fa| !fa.contains(a))
                && mother_alleles.as_ref().is_none_or(|ma| !ma.contains(a))
        });
        let kind = if denovo {
            "de_novo_candidate"
        } else {
            "mendelian_inconsistency"
        };
        let explanation = if denovo {
            "子代表现为候选 de novo；可接受为突变或修订关系，原始证据仍保留。"
        } else {
            "子代等位无法由已知父母传递组合解释。"
        };
        Some(make_hard(
            rel,
            variant,
            kind,
            observations,
            repairs,
            explanation,
        ))
    }
}

fn mendelian_repairs(
    child: &[String],
    father: Option<&Vec<String>>,
    mother: Option<&Vec<String>>,
    cg: &Genotype,
    fg: Option<&Genotype>,
    mg: Option<&Genotype>,
) -> Vec<Vec<String>> {
    let consistent = |child: Option<&[String]>,
                      father: Option<&Vec<String>>,
                      mother: Option<&Vec<String>>|
     -> bool {
        let Some(child) = child else {
            return true;
        };
        match (father, mother) {
            (Some(fa), Some(ma)) => fa.iter().any(|from_father| {
                ma.iter().any(|from_mother| {
                    let mut inherited = [from_father.clone(), from_mother.clone()];
                    inherited.sort();
                    let mut observed = child.to_vec();
                    observed.sort();
                    inherited.as_slice() == observed.as_slice()
                })
            }),
            (Some(parent), None) | (None, Some(parent)) => parent.iter().any(|a| child.contains(a)),
            _ => true,
        }
    };
    if consistent(Some(child), father, mother) {
        return Vec::new();
    }
    let mut repairs = Vec::new();
    if consistent(None, father, mother) {
        repairs.push(vec![cg.observation_id.clone()]);
    }
    if let Some(g) = fg {
        if consistent(Some(child), None, mother) {
            repairs.push(vec![g.observation_id.clone()]);
        }
    }
    if let Some(g) = mg {
        if consistent(Some(child), father, None) {
            repairs.push(vec![g.observation_id.clone()]);
        }
    }
    if repairs.is_empty() {
        if let (Some(f), Some(m)) = (fg, mg) {
            repairs.push(vec![f.observation_id.clone(), m.observation_id.clone()]);
        }
    }
    repairs.sort();
    repairs.dedup();
    repairs
}

fn info_conflict(
    rel: &Relationship,
    variant: &Variant,
    kind: &str,
    explanation: &str,
    observations: Vec<String>,
) -> ConflictView {
    ConflictView {
        conflict_id: stable_id(&format!(
            "{kind}:{}:{}:{}",
            rel.relationship_id,
            variant.id,
            observations.join("|")
        )),
        kind: kind.into(),
        severity: "review".into(),
        chrom: variant.chrom.clone(),
        position: variant.position,
        variant_id: variant.id.clone(),
        relationship_id: rel.relationship_id.clone(),
        observations,
        minimal_repair_sets: Vec::new(),
        explanation: explanation.into(),
        resolved: false,
    }
}

fn make_hard(
    rel: &Relationship,
    variant: &Variant,
    kind: &str,
    observations: Vec<String>,
    repairs: Vec<Vec<String>>,
    explanation: &str,
) -> ConflictView {
    ConflictView {
        conflict_id: stable_id(&format!(
            "{kind}:{}:{}:{}",
            rel.relationship_id,
            variant.id,
            observations.join("|")
        )),
        kind: kind.into(),
        severity: "hard".into(),
        chrom: variant.chrom.clone(),
        position: variant.position,
        variant_id: variant.id.clone(),
        relationship_id: rel.relationship_id.clone(),
        observations,
        minimal_repair_sets: repairs,
        explanation: explanation.into(),
        resolved: false,
    }
}

fn minimum_hitting_sets(repair_options: &[Vec<Vec<String>>], max_nodes: usize) -> Vec<Vec<String>> {
    let options: Vec<BTreeSet<String>> = repair_options
        .iter()
        .map(|opts| opts.iter().flatten().cloned().collect())
        .collect();
    let required: Vec<Vec<BTreeSet<String>>> = repair_options
        .iter()
        .map(|opts| {
            opts.iter()
                .map(|set| set.iter().cloned().collect())
                .collect()
        })
        .collect();
    let mut nodes = 0usize;
    for size in 1..=options.len().max(1) {
        let mut found = Vec::new();
        let universe: BTreeSet<String> = options.iter().flatten().cloned().collect();
        let values: Vec<String> = universe.into_iter().collect();
        #[allow(clippy::too_many_arguments)]
        fn combos(
            values: &[String],
            size: usize,
            start: usize,
            chosen: &mut Vec<String>,
            required: &[Vec<BTreeSet<String>>],
            found: &mut Vec<Vec<String>>,
            nodes: &mut usize,
            max_nodes: usize,
        ) -> bool {
            if *nodes >= max_nodes {
                return false;
            }
            *nodes += 1;
            if chosen.len() == size {
                let chosen_set: BTreeSet<&String> = chosen.iter().collect();
                if required.iter().all(|opts| {
                    opts.iter()
                        .any(|opt| opt.iter().all(|x| chosen_set.contains(x)))
                }) {
                    found.push(chosen.clone());
                }
                return true;
            }
            for i in start..=values.len().saturating_sub(size - chosen.len()) {
                chosen.push(values[i].clone());
                if !combos(
                    values,
                    size,
                    i + 1,
                    chosen,
                    required,
                    found,
                    nodes,
                    max_nodes,
                ) {
                    return false;
                }
                chosen.pop();
            }
            true
        }
        let mut chosen = Vec::new();
        if !combos(
            &values,
            size,
            0,
            &mut chosen,
            &required,
            &mut found,
            &mut nodes,
            max_nodes,
        ) {
            break;
        }
        if !found.is_empty() {
            found.sort();
            found.dedup();
            return found;
        }
    }
    options
        .into_iter()
        .collect::<Vec<_>>()
        .into_iter()
        .map(|s| s.into_iter().collect())
        .collect()
}

fn stable_id(input: &str) -> String {
    let mut hash = 1469598103934665603u64;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{:x}", hash)
}

#[allow(clippy::too_many_arguments)]
fn build_blocks(
    raw: &RawState,
    nodes: &BTreeMap<String, GraphNode>,
    xor: &[XorEdge],
    unary: &[UnaryConstraint],
    physical: &[PhysicalEdge],
    decisions: &Adjudication,
    ignored: &mut BTreeMap<String, IgnoredObservation>,
    ignore: &impl Fn(&mut BTreeMap<String, IgnoredObservation>, &str, &str, &str, f64, bool),
    budget: usize,
) -> Vec<BlockView> {
    let mut components: Vec<BTreeSet<String>> =
        nodes.keys().map(|k| BTreeSet::from([k.clone()])).collect();
    let mut component_of: HashMap<String, usize> = nodes
        .keys()
        .enumerate()
        .map(|(i, k)| (k.clone(), i))
        .collect();
    let mut edges_by_component: BTreeMap<usize, Vec<&PhysicalEdge>> = BTreeMap::new();
    let mut bridges: BTreeMap<usize, Vec<BridgeEvidence>> = BTreeMap::new();

    for edge in physical
        .iter()
        .filter(|e| nodes.contains_key(&e.left) && nodes.contains_key(&e.right))
    {
        let li = component_of[&edge.left];
        let ri = component_of[&edge.right];
        if li != ri {
            let left_min = components[li]
                .iter()
                .min()
                .cloned()
                .unwrap_or_else(|| edge.left.clone());
            let right_min = components[ri]
                .iter()
                .min()
                .cloned()
                .unwrap_or_else(|| edge.right.clone());
            let merged_idx = li.min(ri);
            let absorbed_idx = li.max(ri);
            bridges.entry(merged_idx).or_default().push(BridgeEvidence {
                bridge_id: format!("bridge:{}", edge.id),
                observation_id: edge.id.clone(),
                kind: edge.kind.clone(),
                left_block: format!("block:{left_min}"),
                right_block: format!("block:{right_min}"),
            });
            let absorbed = components[absorbed_idx].clone();
            let absorbed_edges = edges_by_component.remove(&absorbed_idx).unwrap_or_default();
            let absorbed_bridges = bridges.remove(&absorbed_idx).unwrap_or_default();
            components[merged_idx].extend(absorbed);
            edges_by_component
                .entry(merged_idx)
                .or_default()
                .extend(absorbed_edges);
            bridges
                .entry(merged_idx)
                .or_default()
                .extend(absorbed_bridges);
            for key in &components[merged_idx] {
                component_of.insert(key.clone(), merged_idx);
            }
            components[absorbed_idx].clear();
        }
        edges_by_component
            .entry(component_of[&edge.left])
            .or_default()
            .push(edge);
    }

    let mut blocks = Vec::new();
    for (idx, component) in components.into_iter().enumerate() {
        if component.is_empty() {
            continue;
        }
        let mut members: Vec<&GraphNode> = component.iter().map(|k| &nodes[k]).collect();
        members.sort_by(|a, b| {
            let va = &raw.variants[&a.variant];
            let vb = &raw.variants[&b.variant];
            va.chrom
                .cmp(&vb.chrom)
                .then(va.position.cmp(&vb.position))
                .then(b.sample.cmp(&a.sample))
                .then(a.key.cmp(&b.key))
        });
        let member_set: BTreeSet<&str> = component.iter().map(String::as_str).collect();
        let component_edges: Vec<XorEdge> = xor
            .iter()
            .filter(|e| {
                member_set.contains(e.left.as_str()) && member_set.contains(e.right.as_str())
            })
            .cloned()
            .collect();
        let component_unary: Vec<UnaryConstraint> = unary
            .iter()
            .filter(|u| member_set.contains(u.node.as_str()))
            .cloned()
            .collect();
        for edge in &component_edges {
            if !component.iter().any(|k| k == &edge.left)
                || !component.iter().any(|k| k == &edge.right)
            {
                ignore(
                    ignored,
                    &edge.id,
                    "edge_cross_component",
                    "跨染色体或无物理连接的软证据仅列出，不合并 block",
                    edge.weight,
                    false,
                );
            }
        }

        let ordered_nodes: Vec<String> = members.iter().map(|n| n.key.clone()).collect();
        let hard_locks: BTreeMap<String, u8> = ordered_nodes
            .iter()
            .filter_map(|key| {
                let parts = key.split_once(':')?;
                let origin = decisions
                    .locks
                    .get(key)
                    .or_else(|| decisions.locks.get(&format!("{}:{}", parts.0, parts.1)))?;
                Some((key.clone(), if origin.0 == "father" { 0 } else { 1 }))
            })
            .collect();
        let solved = solve_component(
            &ordered_nodes,
            &component_edges,
            &component_unary,
            &hard_locks,
            budget,
        );
        let accepted_signature = ordered_nodes.iter().min().and_then(|min_node| {
            let block_id = format!("block:{min_node}");
            decisions.accepted.get(&block_id).map(|v| v.0.clone())
        });

        let mut candidates = Vec::new();
        for (index, candidate) in solved.candidates.iter().enumerate() {
            let signature = candidate_signature(&ordered_nodes, &candidate.assignment);
            let mut supports = Vec::new();
            for edge in &component_edges {
                supports.push(EvidenceRef {
                    observation_id: edge.id.clone(),
                    kind: edge.kind.clone(),
                    weight: edge.weight,
                    satisfied: candidate.satisfied.contains(&edge.id),
                });
            }
            for item in &component_unary {
                supports.push(EvidenceRef {
                    observation_id: item.id.clone(),
                    kind: item.kind.clone(),
                    weight: item.weight,
                    satisfied: candidate.satisfied.contains(&item.id),
                });
            }
            supports.sort_by(|a, b| {
                a.observation_id
                    .cmp(&b.observation_id)
                    .then(a.kind.cmp(&b.kind))
            });
            let mut candidate_ignored: Vec<IgnoredObservation> =
                candidate.ignored.values().cloned().collect();
            candidate_ignored.sort_by(|a, b| a.observation_id.cmp(&b.observation_id));
            candidates.push(CandidateView {
                candidate_id: format!("candidate:{}", index + 1),
                signature: signature.clone(),
                score: candidate.score,
                orientation: orient_for_nodes(&ordered_nodes, &candidate.assignment),
                supports,
                ignored_observations: candidate_ignored,
                label_swap_equivalent: !component_unary.iter().any(|u| u.weight >= 2.0)
                    && solved.candidates.len() > 1,
                accepted: accepted_signature.as_deref() == Some(signature.as_str()),
            });
        }

        let min_node = ordered_nodes.first().cloned().unwrap_or_default();
        let chrom = members
            .first()
            .and_then(|n| raw.variants.get(&n.variant))
            .map(|v| v.chrom.clone())
            .unwrap_or_default();
        blocks.push(BlockView {
            block_id: format!("block:{min_node}"),
            chrom,
            nodes: members
                .into_iter()
                .map(|n| NodeView {
                    sample_id: n.sample.clone(),
                    variant_id: n.variant.clone(),
                    allele0: n.alleles[0].clone(),
                    allele1: n.alleles[1].clone(),
                })
                .collect(),
            candidates,
            bridge_evidence: bridges.remove(&idx).unwrap_or_default(),
            budget_reached: solved.budget_reached,
            search_incomplete: solved.search_incomplete,
            no_unique_solution: solved.candidates.len() != 1,
        });
    }
    blocks.sort_by(|a, b| a.block_id.cmp(&b.block_id));
    blocks
}

struct Solved {
    candidates: Vec<Candidate>,
    budget_reached: bool,
    search_incomplete: bool,
}

fn solve_component(
    ordered_nodes: &[String],
    edges: &[XorEdge],
    unary: &[UnaryConstraint],
    locks: &BTreeMap<String, u8>,
    budget: usize,
) -> Solved {
    let index: BTreeMap<String, usize> = ordered_nodes
        .iter()
        .enumerate()
        .map(|(i, k)| (k.clone(), i))
        .collect();
    let valid_edges: Vec<&XorEdge> = edges
        .iter()
        .filter(|e| index.contains_key(&e.left) && index.contains_key(&e.right))
        .collect();
    let valid_unary: Vec<&UnaryConstraint> = unary
        .iter()
        .filter(|u| index.contains_key(&u.node))
        .collect();
    let total_possible: f64 = valid_edges.iter().map(|e| e.weight).sum::<f64>()
        + valid_unary
            .iter()
            .filter(|u| !locks.contains_key(&u.node) || Some(&u.bit) == locks.get(&u.node))
            .map(|u| u.weight)
            .sum::<f64>();
    let mut best = Vec::new();
    let mut best_score = f64::NEG_INFINITY;
    let mut visited = 0usize;
    let mut budget_reached = false;

    let mut assignment = BTreeMap::new();

    #[allow(clippy::too_many_arguments)]
    fn visit(
        depth: usize,
        nodes: &[String],
        edges: &[&XorEdge],
        unary: &[&UnaryConstraint],
        locks: &BTreeMap<String, u8>,
        budget: usize,
        assignment: &mut BTreeMap<String, u8>,
        best: &mut Vec<Candidate>,
        best_score: &mut f64,
        visited: &mut usize,
        budget_reached: &mut bool,
        total_possible: f64,
    ) {
        if *budget_reached {
            return;
        }
        if *visited >= budget.max(1) {
            *budget_reached = true;
            return;
        }
        if depth == nodes.len() {
            *visited += 1;
            let mut score = 0.0;
            let mut satisfied = BTreeSet::new();
            let mut ignored: BTreeMap<String, IgnoredObservation> = BTreeMap::new();
            for edge in edges {
                let ok = (*assignment.get(&edge.left).unwrap()
                    ^ *assignment.get(&edge.right).unwrap())
                    == edge.parity;
                if ok {
                    score += edge.weight;
                    satisfied.insert(edge.id.clone());
                } else {
                    ignored.insert(
                        edge.id.clone(),
                        IgnoredObservation {
                            observation_id: edge.id.clone(),
                            kind: edge.kind.clone(),
                            reason: "候选相位与该软约束冲突，因此隔离并忽略此观测".into(),
                            weight: edge.weight,
                            candidate_specific: true,
                        },
                    );
                }
            }
            for item in unary {
                if locks.contains_key(&item.node) && Some(&item.bit) != locks.get(&item.node) {
                    return;
                }
                let ok = assignment.get(&item.node) == Some(&item.bit);
                if ok {
                    score += item.weight;
                    satisfied.insert(item.id.clone());
                } else {
                    ignored.insert(
                        item.id.clone(),
                        IgnoredObservation {
                            observation_id: item.id.clone(),
                            kind: item.kind.clone(),
                            reason: "候选相位与来源锚定冲突，该候选显式忽略此软证据".into(),
                            weight: item.weight,
                            candidate_specific: true,
                        },
                    );
                }
            }
            if score > *best_score {
                *best_score = score;
                best.clear();
                best.push(Candidate {
                    assignment: assignment.clone(),
                    score,
                    satisfied,
                    ignored,
                });
            } else if score == *best_score {
                best.push(Candidate {
                    assignment: assignment.clone(),
                    score,
                    satisfied,
                    ignored,
                });
            }
            return;
        }

        let partial = partial_score(assignment, edges, unary, locks, depth, nodes);
        if partial.is_none() {
            return;
        }
        if partial.unwrap() + total_possible + 0.0000001 < *best_score {
            return;
        }
        let key = nodes[depth].clone();
        for bit in 0..=1u8 {
            if let Some(locked) = locks.get(&key) {
                if bit != *locked {
                    continue;
                }
            }
            assignment.insert(key.clone(), bit);
            visit(
                depth + 1,
                nodes,
                edges,
                unary,
                locks,
                budget,
                assignment,
                best,
                best_score,
                visited,
                budget_reached,
                total_possible,
            );
            assignment.remove(&key);
        }
    }

    visit(
        0,
        ordered_nodes,
        &valid_edges,
        &valid_unary,
        locks,
        budget.max(1),
        &mut assignment,
        &mut best,
        &mut best_score,
        &mut visited,
        &mut budget_reached,
        total_possible,
    );
    if locks.values().all(|bit| *bit == 0 || *bit == 1)
        && ordered_nodes.iter().any(|n| !locks.contains_key(n))
    {
        // 未锁定部分仍可能有等价翻转；预算截断时不得宣布唯一。
    }
    best.sort_by(|a, b| a.assignment.cmp(&b.assignment));
    best.dedup_by(|a, b| a.assignment == b.assignment);
    let search_incomplete = budget_reached || best.len() > 1;
    Solved {
        candidates: best,
        budget_reached,
        search_incomplete,
    }
}

fn partial_score(
    assignment: &BTreeMap<String, u8>,
    edges: &[&XorEdge],
    unary: &[&UnaryConstraint],
    locks: &BTreeMap<String, u8>,
    _depth: usize,
    _nodes: &[String],
) -> Option<f64> {
    let mut score = 0.0;
    for edge in edges {
        if let (Some(a), Some(b)) = (assignment.get(&edge.left), assignment.get(&edge.right)) {
            if (a ^ b) == edge.parity {
                score += edge.weight;
            }
        }
    }
    for item in unary {
        if let Some(bit) = assignment.get(&item.node) {
            if let Some(locked) = locks.get(&item.node) {
                if locked != bit {
                    return None;
                }
            } else if *bit == item.bit {
                score += item.weight;
            }
        }
    }
    Some(score)
}

fn candidate_signature(nodes: &[String], assignment: &BTreeMap<String, u8>) -> String {
    let bits: String = nodes
        .iter()
        .map(|n| char::from_digit(*assignment.get(n).unwrap_or(&0) as u32, 10).unwrap_or('?'))
        .collect();
    format!("sig:{}", stable_id(&format!("{}|{bits}", nodes.join(","))))
}

fn orient_for_nodes(nodes: &[String], assignment: &BTreeMap<String, u8>) -> BTreeMap<String, u8> {
    nodes
        .iter()
        .filter_map(|n| assignment.get(n).map(|bit| (n.clone(), *bit)))
        .collect()
}

fn finalize_decisions(
    adjudication: &Adjudication,
    state: &State,
    analysis: &Analysis,
) -> Vec<DecisionView> {
    let active_block_ids: BTreeSet<String> =
        analysis.blocks.iter().map(|b| b.block_id.clone()).collect();
    let active_link_ids: BTreeSet<String> =
        state.read_links.iter().map(|l| l.link_id.clone()).collect();
    let active_rel_ids: BTreeSet<String> = state
        .relationships
        .iter()
        .map(|r| r.relationship_id.clone())
        .collect();
    adjudication
        .decisions
        .iter()
        .map(|(event, effective, _)| {
            let mut stale = false;
            let mut reason = None;
            if *effective {
                match event.kind.as_str() {
                    "accept_candidate" => {
                        let block_id = str_field(event, "block_id").unwrap_or_default();
                        let signature = str_field(event, "candidate_signature").unwrap_or_default();
                        if let Some(block) = analysis.blocks.iter().find(|b| b.block_id == block_id)
                        {
                            if !block.candidates.iter().any(|c| c.signature == signature) {
                                stale = true;
                                reason = Some("候选签名已因新证据、撤回或版本变化失效".into());
                            }
                        } else if active_block_ids.contains(&block_id) {
                            stale = true;
                            reason = Some("候选签名未出现在当前 block".into());
                        } else {
                            stale = true;
                            reason = Some("block 已因桥接撤回或合并改变".into());
                        }
                    }
                    "withdraw_read_link" => {
                        let id = str_field(event, "read_link_id").unwrap_or_default();
                        if !active_link_ids.contains(&id) {
                            stale = true;
                            reason = Some("read link 不在当前钉住版本".into());
                        }
                    }
                    "revise_relationship" | "mark_relationship_uncertain" => {
                        let id = str_field(event, "relationship_id").unwrap_or_default();
                        if !active_rel_ids.contains(&id) {
                            stale = true;
                            reason = Some("关系已被当前版本替换或不存在".into());
                        }
                    }
                    _ => {}
                }
                if let Some(version) = event.input_version {
                    if version != state.effective_version {
                        stale = true;
                        reason = Some(format!(
                            "裁定基于旧版本 {version}；当前有效版本为 {}",
                            state.effective_version
                        ));
                    }
                }
            }
            DecisionView {
                event_id: event.id,
                kind: event.kind.clone(),
                payload: event.payload.clone(),
                input_version: event.input_version,
                effective: *effective,
                stale,
                stale_reason: reason,
            }
        })
        .collect()
}

pub fn export_state(state: &State) -> Value {
    json!({
        "application": "相位织图",
        "offline": true,
        "external_reference_lookup": false,
        "pinned_input_version": state.pinned_version,
        "effective_input_version": state.effective_version,
        "branch": state.branch,
        "decisions": state.decisions,
        "blocks": state.blocks,
        "conflicts": state.conflicts,
        "replay_steps": state.replay_steps,
    })
}
