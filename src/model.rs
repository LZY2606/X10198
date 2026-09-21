use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sample {
    pub id: String,
    pub cohort: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relation {
    pub child: String,
    pub parent: String,
    pub role: String,   // "father" | "mother"
    pub status: String, // "confirmed" | "to_confirm"
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Variant {
    pub chrom: String,
    pub pos: i64,
    pub ref_a: String,
    pub alt_a: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenotypeObs {
    pub obs_id: i64,
    pub sample: String,
    pub chrom: String,
    pub pos: i64,
    pub a1: Option<u8>,
    pub a2: Option<u8>,
    pub gq: Option<f64>,
    pub batch: String,
    pub version: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Orientation {
    Cis,
    Trans,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadLink {
    pub id: i64,
    pub sample: String,
    pub chrom: String,
    pub pos1: i64,
    pub pos2: i64,
    pub orientation: Orientation,
    pub weight: f64,
    pub version: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Decision {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub kind: String,
    pub payload: serde_json::Value,
    pub base_version: i64,
    pub stale: bool,
    pub created_at: i64,
}

// ---- import payload (POST /api/import) ----

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ImportBatch {
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub samples: Vec<SampleIn>,
    #[serde(default)]
    pub relations: Vec<RelationIn>,
    #[serde(default)]
    pub variants: Vec<VariantIn>,
    #[serde(default)]
    pub genotypes: Vec<GenotypeIn>,
    #[serde(default)]
    pub read_links: Vec<ReadLinkIn>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SampleIn {
    pub id: String,
    #[serde(default)]
    pub cohort: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RelationIn {
    pub child: String,
    pub parent: String,
    pub role: String,
    #[serde(default = "default_status")]
    pub status: String,
}

fn default_status() -> String {
    "confirmed".to_string()
}

#[derive(Clone, Debug, Deserialize)]
pub struct VariantIn {
    pub chrom: String,
    pub pos: i64,
    #[serde(default)]
    pub ref_a: Option<String>,
    #[serde(default)]
    pub alt_a: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GenotypeIn {
    pub sample: String,
    pub chrom: String,
    pub pos: i64,
    #[serde(default)]
    pub a1: Option<u8>,
    #[serde(default)]
    pub a2: Option<u8>,
    #[serde(default)]
    pub gq: Option<f64>,
    #[serde(default)]
    pub gl: Option<Vec<f64>>,
    #[serde(default)]
    pub batch: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ReadLinkIn {
    pub sample: String,
    pub chrom: String,
    pub pos1: i64,
    pub pos2: i64,
    pub orientation: String, // "cis" | "trans"
    #[serde(default = "default_weight")]
    pub weight: f64,
}

fn default_weight() -> f64 {
    1.0
}
