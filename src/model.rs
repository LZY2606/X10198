use serde::{Deserialize, Serialize};

/// A sample is only ever shown through a local anonymous label.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Sample {
    pub id: i64,
    pub anon_label: String,
    pub batch: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParentKind {
    Father,
    Mother,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelStatus {
    Confirmed,
    ToConfirm,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Relationship {
    pub id: i64,
    pub child: i64,
    pub parent: i64,
    pub kind: ParentKind,
    pub status: RelStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Variant {
    pub id: i64,
    pub chrom: String,
    pub pos: i64,
    /// Alleles observed at the site; index 0 is the reference.
    /// More than two entries means a multi-allelic (e.g. triallelic) site.
    pub alleles: Vec<String>,
}

/// A raw genotype-likelihood observation. Immutable once imported; the
/// `version` pins the input version at which it arrived.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Observation {
    pub id: i64,
    pub sample: i64,
    pub variant: i64,
    /// Plain likelihoods over unordered genotypes (i<=j), index j*(j+1)/2+i.
    pub gls: Vec<f64>,
    pub quality: f64,
    pub batch: String,
    pub version: i64,
}

/// A read-level phase link between two variants of one sample.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReadLink {
    pub id: i64,
    pub sample: i64,
    pub var_a: i64,
    pub var_b: i64,
    /// true = the two reference alleles were seen on the same molecule.
    pub same_haplotype: bool,
    pub weight: f64,
    pub retracted: bool,
    pub batch: String,
    pub version: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    MendelianInconsistent,
    MissingGenotype,
    LowQualityHet,
    DuplicateSample,
    DeNovoCandidate,
    UncertainParentage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Conflict {
    pub kind: ConflictKind,
    pub sample: Option<i64>,
    pub variant: Option<i64>,
    pub detail: String,
    /// Observation ids that carry the conflicting evidence.
    pub evidence: Vec<i64>,
    /// Minimal set of observation ids whose exclusion restores consistency.
    pub minimal_ignore: Vec<i64>,
}

/// A weighted phase relation between two heterozygous variants of a sample.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhaseEdge {
    pub sample: i64,
    pub var_a: i64,
    pub var_b: i64,
    /// true = allele-0 of both variants phased to the same haplotype.
    pub same: bool,
    pub weight: f64,
    pub source: EdgeSource,
    /// Observation / read-link ids backing this relation.
    pub support: Vec<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeSource {
    Transmission,
    ReadLink,
    Locked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Candidate {
    pub index: usize,
    pub score: f64,
    /// For each variant position in the block: true = allele 0 on haplotype A.
    pub orientation: Vec<bool>,
    /// (edge summary) relations satisfied by this candidate.
    pub supporting: Vec<PhaseEdge>,
    /// Data this candidate ignores (contradicting edges, low-quality calls,
    /// minimal-ignore observations).
    pub ignored: Vec<String>,
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhaseBlock {
    pub id: String,
    pub sample: i64,
    pub chrom: String,
    pub variant_ids: Vec<i64>,
    pub start_pos: i64,
    pub end_pos: i64,
    /// Read-link ids that bridged smaller blocks into this one.
    pub bridge_evidence: Vec<i64>,
    pub candidates: Vec<Candidate>,
    /// True when the candidate budget was exhausted: the result must not be
    /// presented as the unique solution.
    pub budget_hit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Decision {
    AcceptCandidate { block_id: String, candidate_index: usize },
    LockPhase {
        sample: i64,
        chrom: String,
        from_pos: i64,
        to_pos: i64,
        /// For each variant id in range: allele-0 on haplotype A?
        orientation: Vec<(i64, bool)>,
    },
    MarkRelationship { relationship_id: i64, status: RelStatus },
    RetractReadLink { link_id: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionEvent {
    pub seq: i64,
    pub branch: String,
    pub input_version: i64,
    pub decision: Decision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportBundle {
    #[serde(default)]
    pub samples: Vec<Sample>,
    #[serde(default)]
    pub relationships: Vec<Relationship>,
    #[serde(default)]
    pub variants: Vec<Variant>,
    #[serde(default)]
    pub observations: Vec<Observation>,
    #[serde(default)]
    pub read_links: Vec<ReadLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportBundle {
    pub input_version: i64,
    pub pinned_batches: Vec<String>,
    pub events: Vec<DecisionEvent>,
}
