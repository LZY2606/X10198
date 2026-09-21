use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportPayload {
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub candidate_budget: Option<usize>,
    pub observations: Vec<ObservationInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObservationInput {
    Sample(SampleInput),
    Relationship(RelationshipInput),
    Variant(VariantInput),
    Genotype(GenotypeInput),
    ReadLink(ReadLinkInput),
}

impl ObservationInput {
    pub fn id(&self) -> &str {
        match self {
            ObservationInput::Sample(v) => &v.id,
            ObservationInput::Relationship(v) => &v.id,
            ObservationInput::Variant(v) => &v.id,
            ObservationInput::Genotype(v) => &v.id,
            ObservationInput::ReadLink(v) => &v.id,
        }
    }

    pub fn batch(&self) -> Option<&str> {
        match self {
            ObservationInput::Sample(v) => Some(&v.batch),
            ObservationInput::Relationship(v) => v.batch.as_deref(),
            ObservationInput::Variant(v) => Some(&v.batch),
            ObservationInput::Genotype(v) => Some(&v.batch),
            ObservationInput::ReadLink(v) => Some(&v.batch),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SampleInput {
    pub id: String,
    pub anonymous_label: String,
    pub batch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationshipInput {
    pub id: String,
    pub child: String,
    #[serde(default)]
    pub father: Option<String>,
    #[serde(default)]
    pub mother: Option<String>,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub batch: Option<String>,
}

fn default_confidence() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VariantInput {
    pub id: String,
    pub chromosome: String,
    pub position: i64,
    pub reference_allele: String,
    pub alternate_alleles: Vec<String>,
    pub batch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenotypeInput {
    pub id: String,
    pub sample: String,
    pub variant: String,
    #[serde(default)]
    pub alleles: Vec<String>,
    #[serde(default)]
    pub likelihoods: BTreeMap<String, f64>,
    #[serde(default = "default_quality")]
    pub quality: f64,
    #[serde(default)]
    pub missing: bool,
    #[serde(default)]
    pub low_quality_heterozygous: bool,
    #[serde(default)]
    pub duplicate_observation: bool,
    pub batch: String,
}

fn default_quality() -> f64 {
    30.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReadLinkInput {
    pub id: String,
    pub sample: String,
    pub chromosome: String,
    pub left_variant: String,
    pub right_variant: String,
    pub left_allele: String,
    pub right_allele: String,
    #[serde(default = "default_link_weight")]
    pub weight: f64,
    #[serde(default)]
    pub withdrawn: bool,
    pub batch: String,
}

fn default_link_weight() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sample {
    pub id: String,
    pub anonymous_label: String,
    pub batch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Relationship {
    pub id: String,
    pub child: String,
    pub father: Option<String>,
    pub mother: Option<String>,
    pub confidence: f64,
    pub status: String,
    pub batch: Option<String>,
}

impl Relationship {
    pub fn is_uncertain(&self) -> bool {
        self.confidence < 0.95 || self.status == "uncertain" || self.status == "parent_identity_uncertain"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Variant {
    pub id: String,
    pub chromosome: String,
    pub position: i64,
    pub reference_allele: String,
    pub alternate_alleles: Vec<String>,
    pub batch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Genotype {
    pub id: String,
    pub sample: String,
    pub variant: String,
    pub alleles: Vec<String>,
    pub likelihoods: BTreeMap<String, f64>,
    pub quality: f64,
    pub missing: bool,
    pub low_quality_heterozygous: bool,
    pub duplicate_observation: bool,
    pub batch: String,
}

impl Genotype {
    pub fn diploid(&self) -> bool {
        !self.missing && self.alleles.len() == 2
    }

    pub fn unique_alleles(&self) -> Vec<String> {
        let mut alleles = self.alleles.clone();
        alleles.sort();
        alleles.dedup();
        alleles
    }

    pub fn heterozygous(&self) -> bool {
        self.diploid() && self.unique_alleles().len() == 2
    }

    pub fn triallelic(&self) -> bool {
        !self.missing && self.unique_alleles().len() > 2
    }

    pub fn can_supply(&self, allele: &str) -> bool {
        (self.missing || self.alleles.is_empty()) || self.alleles.iter().any(|value| value == allele)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReadLink {
    pub id: String,
    pub sample: String,
    pub chromosome: String,
    pub left_variant: String,
    pub right_variant: String,
    pub left_allele: String,
    pub right_allele: String,
    pub weight: f64,
    pub withdrawn: bool,
    pub batch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalysisInput {
    pub version_id: String,
    pub note: Option<String>,
    pub candidate_budget: usize,
    pub samples: Vec<Sample>,
    pub relationships: Vec<Relationship>,
    pub variants: Vec<Variant>,
    pub genotypes: Vec<Genotype>,
    pub read_links: Vec<ReadLink>,
}

impl AnalysisInput {
    pub fn genotype(&self, sample_id: &str, variant_id: &str) -> Option<&Genotype> {
        self.genotypes
            .iter()
            .find(|item| item.sample == sample_id && item.variant == variant_id && !item.duplicate_observation)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Decision {
    AcceptCandidate {
        block_id: String,
        candidate_id: String,
    },
    LockPhase {
        block_id: String,
        variant_id: String,
        allele_a: String,
        allele_b: String,
    },
    MarkRelationshipUncertain {
        relationship_id: String,
        note: Option<String>,
    },
    ReviseRelationship {
        relationship_id: String,
        father: Option<String>,
        mother: Option<String>,
        confidence: f64,
        note: Option<String>,
    },
    WithdrawReadLink {
        read_link_id: String,
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DecisionOverlay {
    pub decisions: Vec<Decision>,
    pub withdrawn_links: Vec<String>,
    pub accepted_candidates: BTreeMap<String, String>,
    pub locked_phases: BTreeMap<String, LockedPhase>,
    pub uncertain_relationships: BTreeMap<String, String>,
    pub revised_relationships: BTreeMap<String, RelationshipRevision>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockedPhase {
    pub variant_id: String,
    pub allele_a: String,
    pub allele_b: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationshipRevision {
    pub father: Option<String>,
    pub mother: Option<String>,
    pub confidence: f64,
    pub note: Option<String>,
}
