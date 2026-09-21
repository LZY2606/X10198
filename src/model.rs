use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelationStatus {
    Confirmed,
    Uncertain,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    High,
    Low,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sample {
    pub id: String,
    pub anon_id: String,
    pub batch: String,
    #[serde(default)]
    pub duplicate_group: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Relationship {
    pub id: String,
    pub child_id: String,
    pub father_id: String,
    pub mother_id: String,
    pub status: RelationStatus,
    pub batch: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Variant {
    pub id: String,
    pub chrom: String,
    pub pos: i64,
    pub alleles: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GenotypeObs {
    pub id: String,
    pub sample_id: String,
    pub variant_id: String,
    pub calls: Vec<String>,
    pub likelihood: f64,
    pub quality: Quality,
    pub batch: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReadLinkObs {
    pub id: String,
    pub sample_id: String,
    pub left_variant_id: String,
    pub right_variant_id: String,
    pub left_allele: String,
    pub right_allele: String,
    pub weight: f64,
    pub batch: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InputSnapshot {
    pub version: String,
    #[serde(default)]
    pub note: String,
    pub samples: Vec<Sample>,
    pub relationships: Vec<Relationship>,
    pub variants: Vec<Variant>,
    pub genotypes: Vec<GenotypeObs>,
    pub read_links: Vec<ReadLinkObs>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AssignmentEntry {
    pub sample_id: String,
    pub variant_id: String,
    pub allele: String,
    pub haplotype: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DecisionEvent {
    AcceptCandidate {
        input_version: String,
        block_id: String,
        candidate_id: String,
        note: String,
    },
    LockPhase {
        input_version: String,
        sample_id: String,
        chrom: String,
        start: i64,
        end: i64,
        assignment: Vec<AssignmentEntry>,
        note: String,
    },
    WithdrawReadLink {
        input_version: String,
        link_id: String,
        reason: String,
    },
    SetRelationshipStatus {
        input_version: String,
        relationship_id: String,
        status: RelationStatus,
        reason: String,
    },
}

impl DecisionEvent {
    pub fn input_version(&self) -> &str {
        match self {
            DecisionEvent::AcceptCandidate { input_version, .. }
            | DecisionEvent::LockPhase { input_version, .. }
            | DecisionEvent::WithdrawReadLink { input_version, .. }
            | DecisionEvent::SetRelationshipStatus { input_version, .. } => input_version,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventRecord {
    InputImported(InputSnapshot),
    Decision(DecisionEvent),
}

impl EventRecord {
    pub fn event_type(&self) -> &'static str {
        match self {
            EventRecord::InputImported(_) => "input_imported",
            EventRecord::Decision(_) => "decision",
        }
    }

    pub fn input_version(&self) -> Option<&str> {
        match self {
            EventRecord::InputImported(input) => Some(input.version.as_str()),
            EventRecord::Decision(decision) => Some(decision.input_version()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DecisionOverlay {
    pub withdrawn_links: Vec<(String, String)>,
    pub accepted: Vec<AcceptedCandidate>,
    pub locks: Vec<PhaseLock>,
    pub relationship_status: BTreeMap<String, RelationStatus>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AcceptedCandidate {
    pub block_id: String,
    pub candidate_id: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhaseLock {
    pub sample_id: String,
    pub chrom: String,
    pub start: i64,
    pub end: i64,
    pub assignment: Vec<AssignmentEntry>,
    pub note: String,
}
