use crate::model::*;

pub fn demo_payload() -> ImportPayload {
    let observations = vec![
        sample("s-father", "本地样本 P-A", "batch-a"),
        sample("s-mother", "本地样本 P-B", "batch-a"),
        sample("s-child", "本地样本 C-1", "batch-b"),
        sample("s-aunt", "本地样本 R-2", "batch-b"),
        ObservationInput::Relationship(RelationshipInput {
            id: "rel-child".into(), child: "s-child".into(),
            father: Some("s-father".into()), mother: Some("s-mother".into()),
            confidence: 0.99, status: Some("confirmed".into()), batch: Some("batch-a".into()),
        }),
        ObservationInput::Relationship(RelationshipInput {
            id: "rel-aunt-uncertain".into(), child: "s-aunt".into(),
            father: Some("s-father".into()), mother: None,
            confidence: 0.72, status: Some("parent_identity_uncertain".into()), batch: Some("batch-b".into()),
        }),
        variant("v1", "chr1", 100, "A", "G", "batch-a"),
        variant("v2", "chr1", 200, "C", "T", "batch-a"),
        variant("v3", "chr1", 300, "G", "A", "batch-b"),
        variant("v4", "chr1", 400, "T", "C", "batch-b"),
        variant("v5", "chr2", 50, "A", "T", "batch-b"),
        genotype("g-father-v1", "s-father", "v1", ["A", "G"], 88.0, false, false),
        genotype("g-mother-v1", "s-mother", "v1", ["A", "A"], 90.0, false, false),
        genotype("g-child-v1", "s-child", "v1", ["A", "G"], 77.0, false, false),
        genotype("g-aunt-v1", "s-aunt", "v1", ["A", "G"], 42.0, true, false),
        genotype("g-father-v2", "s-father", "v2", ["C", "T"], 84.0, false, false),
        genotype("g-mother-v2", "s-mother", "v2", ["T", "T"], 86.0, false, false),
        genotype("g-child-v2", "s-child", "v2", ["C", "T"], 80.0, false, false),
        genotype("g-father-v3", "s-father", "v3", ["G", "G"], 76.0, false, false),
        genotype("g-mother-v3", "s-mother", "v3", ["G", "G"], 74.0, false, false),
        genotype("g-child-v3", "s-child", "v3", ["G", "A"], 66.0, false, false),
        genotype("g-child-v4-missing", "s-child", "v4", [], 0.0, true, false),
        genotype("g-father-v5", "s-father", "v5", ["A", "A"], 90.0, false, false),
        genotype("g-mother-v5", "s-mother", "v5", ["T", "T"], 90.0, false, false),
        genotype("g-child-v5-triallelic", "s-child", "v5", ["A", "T", "C"], 20.0, false, false),
        link("read-father-v1-v2", "s-father", "v1", "v2", "G", "T", 2.0),
        link("read-child-v1-v2", "s-child", "v1", "v2", "G", "C", 2.5),
    ];
    ImportPayload {
        version_id: Some("demo-v1".into()),
        note: Some("内置离线匿名合成小家系".into()),
        candidate_budget: Some(8),
        observations,
    }
}

fn sample(id: &str, label: &str, batch: &str) -> ObservationInput {
    ObservationInput::Sample(SampleInput { id: id.into(), anonymous_label: label.into(), batch: batch.into() })
}

fn variant(id: &str, chromosome: &str, position: i64, reference: &str, alternate: &str, batch: &str) -> ObservationInput {
    ObservationInput::Variant(VariantInput {
        id: id.into(), chromosome: chromosome.into(), position,
        reference_allele: reference.into(), alternate_alleles: vec![alternate.into()], batch: batch.into(),
    })
}

fn genotype<const N: usize>(id: &str, sample: &str, variant: &str, alleles: [&str; N], quality: f64, missing: bool, duplicate: bool) -> ObservationInput {
    ObservationInput::Genotype(GenotypeInput {
        id: id.into(), sample: sample.into(), variant: variant.into(),
        alleles: alleles.into_iter().map(str::to_string).collect(),
        likelihoods: std::collections::BTreeMap::new(), quality, missing,
        low_quality_heterozygous: quality < 50.0 && !missing,
        duplicate_observation: duplicate, batch: "batch-b".into(),
    })
}

fn link(id: &str, sample: &str, left: &str, right: &str, left_allele: &str, right_allele: &str, weight: f64) -> ObservationInput {
    ObservationInput::ReadLink(ReadLinkInput {
        id: id.into(), sample: sample.into(), chromosome: "chr1".into(),
        left_variant: left.into(), right_variant: right.into(),
        left_allele: left_allele.into(), right_allele: right_allele.into(),
        weight, withdrawn: false, batch: "batch-b".into(),
    })
}
