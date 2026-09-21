use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ImportPayload {
    pub version: String,
    #[serde(default)]
    pub batch: String,
    #[serde(default)]
    pub samples: Vec<SampleInput>,
    #[serde(default)]
    pub relationships: Vec<RelationshipInput>,
    #[serde(default)]
    pub variants: Vec<VariantInput>,
    #[serde(default)]
    pub genotypes: Vec<GenotypeInput>,
    #[serde(default)]
    pub read_links: Vec<ReadLinkInput>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SampleInput {
    pub id: String,
    #[serde(default)]
    pub batch: String,
    #[serde(default)]
    pub role_note: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RelationshipInput {
    pub id: String,
    pub child_id: String,
    #[serde(default)]
    pub father_id: Option<String>,
    #[serde(default)]
    pub mother_id: Option<String>,
    #[serde(default)]
    pub batch: String,
    #[serde(default)]
    pub uncertain: bool,
    #[serde(default)]
    pub weight: i64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct VariantInput {
    pub id: String,
    pub chromosome: String,
    pub position: i64,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub alternate: String,
    #[serde(default)]
    pub batch: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GenotypeInput {
    pub id: String,
    pub sample_id: String,
    pub variant_id: String,
    #[serde(default)]
    pub alleles: Vec<String>,
    #[serde(default)]
    pub missing: bool,
    #[serde(default)]
    pub quality: Option<f64>,
    #[serde(default)]
    pub genotype_likelihoods: Vec<f64>,
    #[serde(default)]
    pub batch: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ReadLinkInput {
    pub id: String,
    pub genotype_a: String,
    pub genotype_b: String,
    #[serde(default)]
    pub orientation: String,
    #[serde(default)]
    pub weight: i64,
    #[serde(default)]
    pub quality: Option<f64>,
    #[serde(default)]
    pub batch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: i64,
    pub branch: String,
    pub sequence: i64,
    pub kind: String,
    pub payload: serde_json::Value,
    pub parent_id: Option<i64>,
    pub pinned_version: Option<String>,
    pub created_at: String,
    pub applied: bool,
}
