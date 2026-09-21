use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImportPayload {
    pub batch_id: Option<String>,
    pub samples: Vec<SampleInput>,
    pub variants: Vec<VariantInput>,
    #[serde(default)]
    pub relationships: Vec<RelationshipInput>,
    #[serde(default)]
    pub genotypes: Vec<GenotypeInput>,
    #[serde(default)]
    pub read_links: Vec<ReadLinkInput>,
    #[serde(default)]
    pub transmissions: Vec<TransmissionInput>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SampleInput {
    pub anon_id: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VariantInput {
    pub variant_id: String,
    pub chrom: String,
    pub position: i64,
    pub reference: String,
    #[serde(default)]
    pub alternate: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationshipInput {
    pub relationship_id: Option<String>,
    pub child_id: String,
    #[serde(default)]
    pub father_id: Option<String>,
    #[serde(default)]
    pub mother_id: Option<String>,
    #[serde(default)]
    pub duplicate_of_id: Option<String>,
    #[serde(default = "default_relation_kind")]
    pub kind: String,
    #[serde(default = "default_confidence")]
    pub confidence: String,
    #[serde(default)]
    pub source_batch: String,
}

fn default_relation_kind() -> String {
    "trio".into()
}
fn default_confidence() -> String {
    "confirmed".into()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GenotypeInput {
    #[serde(default)]
    pub observation_id: Option<String>,
    pub sample_id: String,
    pub variant_id: String,
    #[serde(default)]
    pub alleles: Vec<String>,
    #[serde(default)]
    pub is_missing: bool,
    #[serde(default = "default_likelihood")]
    pub likelihood: f64,
    #[serde(default = "default_high")]
    pub quality: String,
    #[serde(default)]
    pub batch_id: String,
}

fn default_likelihood() -> f64 {
    1.0
}
fn default_high() -> String {
    "high".into()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReadLinkInput {
    pub link_id: String,
    pub sample_id: String,
    pub variant_a: String,
    pub variant_b: String,
    pub allele_a_index: usize,
    pub allele_b_index: usize,
    #[serde(default = "default_link_weight")]
    pub weight: f64,
    #[serde(default)]
    pub batch_id: String,
}

fn default_link_weight() -> f64 {
    1.0
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransmissionInput {
    pub transmission_id: String,
    pub child_id: String,
    pub parent_id: String,
    pub parent_role: String,
    pub variant_id: String,
    pub child_allele_index: usize,
    pub parent_allele_index: usize,
    #[serde(default = "default_link_weight")]
    pub weight: f64,
    #[serde(default)]
    pub batch_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DecisionRequest {
    pub kind: String,
    #[serde(default)]
    pub block_id: String,
    #[serde(default)]
    pub candidate_signature: String,
    #[serde(default)]
    pub sample_id: String,
    #[serde(default)]
    pub variant_id: String,
    #[serde(default)]
    pub haplotype_a_origin: String,
    #[serde(default)]
    pub read_link_id: String,
    #[serde(default)]
    pub relationship_id: String,
    #[serde(default)]
    pub conflict_id: String,
    #[serde(default)]
    pub father_id: Option<String>,
    #[serde(default)]
    pub mother_id: Option<String>,
    #[serde(default)]
    pub duplicate_of_id: Option<String>,
    #[serde(default)]
    pub confidence: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub input_version: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BranchRequest {
    pub name: String,
    #[serde(default)]
    pub from_branch: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SwitchRequest {
    pub branch: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RollbackRequest {
    pub event_id: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PinRequest {
    pub version: i64,
}

#[derive(Debug, Serialize)]
pub struct ImportResponse {
    pub version: i64,
    pub batch_id: String,
}
