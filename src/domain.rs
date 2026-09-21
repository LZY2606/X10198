use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputVersion {
    pub id: String,
    pub label: String,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sample {
    pub id: String,
    pub anonymous_id: String,
    pub sex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipStatus {
    Confirmed,
    Pending,
    Withdrawn,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Relationship {
    pub id: String,
    pub child_id: String,
    pub father_id: Option<String>,
    pub mother_id: Option<String>,
    pub status: RelationshipStatus,
    pub weight: i64,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Variant {
    pub id: String,
    pub chromosome: String,
    pub position: i64,
    pub reference: String,
    pub alternate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenotypeObservation {
    pub id: String,
    pub sample_id: String,
    pub variant_id: String,
    pub batch: String,
    pub alleles: Option<Vec<String>>,
    pub likelihoods: Vec<f64>,
    pub quality: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LinkPhase {
    Cis,
    Trans,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadLink {
    pub id: String,
    pub sample_id: String,
    pub variant1_id: String,
    pub variant2_id: String,
    pub phase: LinkPhase,
    pub weight: i64,
    pub batch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputSnapshot {
    pub version: InputVersion,
    pub samples: Vec<Sample>,
    pub relationships: Vec<Relationship>,
    pub variants: Vec<Variant>,
    pub genotypes: Vec<GenotypeObservation>,
    pub read_links: Vec<ReadLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Decision {
    RollbackMarker {
        target_event_id: Option<String>,
        label: String,
    },
    AcceptCandidate {
        block_id: String,
        candidate_id: String,
    },
    LockPhase {
        block_id: String,
        assignments: Vec<PhaseAssignment>,
    },
    MarkRelationshipUncertain {
        relationship_id: String,
        reason: String,
    },
    ReviseRelationship {
        relationship_id: String,
        father_id: Option<String>,
        mother_id: Option<String>,
        status: RelationshipStatus,
        reason: String,
    },
    WithdrawReadLink {
        read_link_id: String,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhaseAssignment {
    pub variant_id: String,
    pub haplotype0_allele: String,
    pub haplotype1_allele: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecisionEvent {
    pub id: String,
    pub seq: i64,
    pub branch_id: String,
    pub parent_event_id: Option<String>,
    pub pinned_version: String,
    pub decision: Decision,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceRef {
    pub observation_id: String,
    pub weight: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateAssignment {
    pub variant_id: String,
    pub haplotype0_allele: String,
    pub haplotype1_allele: String,
    pub origin0: Option<String>,
    pub origin1: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Candidate {
    pub id: String,
    pub block_id: String,
    pub score: i64,
    pub tied: bool,
    pub budget_exhausted: bool,
    pub accepted: bool,
    pub locked: bool,
    pub origin_label_swapped: bool,
    pub assignments: Vec<CandidateAssignment>,
    pub supporting_observations: Vec<EvidenceRef>,
    pub ignored_observations: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeEvidence {
    pub read_link_id: String,
    pub from_component: String,
    pub to_component: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhaseBlock {
    pub id: String,
    pub chromosome: String,
    pub sample_id: String,
    pub variant_ids: Vec<String>,
    pub bridge_evidence: Vec<BridgeEvidence>,
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Conflict {
    pub id: String,
    pub severity: String,
    pub kind: String,
    pub message: String,
    pub involved_observations: Vec<String>,
    pub minimum_restoring_sets: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WithdrawalEffect {
    pub read_link_id: String,
    pub was_bridge: bool,
    pub affected_sample_id: String,
    pub chromosome: String,
    pub current_block_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Analysis {
    pub pinned_version: String,
    pub branch_id: String,
    pub candidate_budget: usize,
    pub samples: Vec<Sample>,
    pub variants: Vec<Variant>,
    pub genotypes: Vec<GenotypeObservation>,
    pub read_links: Vec<ReadLink>,
    pub relationships: Vec<Relationship>,
    pub blocks: Vec<PhaseBlock>,
    pub conflicts: Vec<Conflict>,
    pub effective_events: Vec<DecisionEvent>,
    pub stale_events: Vec<DecisionEvent>,
    pub withdrawal_effects: Vec<WithdrawalEffect>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Branch {
    pub id: String,
    pub parent_branch_id: Option<String>,
    pub created_by_event_id: Option<String>,
    pub label: String,
    pub head_event_id: Option<String>,
}
