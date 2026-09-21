use crate::model::*;

pub fn demo_input() -> InputSnapshot {
    InputSnapshot {
        version: "demo-v1".to_string(),
        note: "离线合成小家系".to_string(),
        samples: vec![
            Sample { id: "F".into(), anon_id: "ANON-F".into(), batch: "b1".into(), duplicate_group: None },
            Sample { id: "M".into(), anon_id: "ANON-M".into(), batch: "b1".into(), duplicate_group: None },
            Sample { id: "C".into(), anon_id: "ANON-C".into(), batch: "b1".into(), duplicate_group: None },
        ],
        relationships: vec![Relationship {
            id: "rel1".into(),
            child_id: "C".into(),
            father_id: "F".into(),
            mother_id: "M".into(),
            status: RelationStatus::Confirmed,
            batch: "b1".into(),
        }],
        variants: vec![
            Variant { id: "v1".into(), chrom: "chr1".into(), pos: 100, alleles: vec!["A".into(), "G".into()] },
            Variant { id: "v2".into(), chrom: "chr1".into(), pos: 200, alleles: vec!["C".into(), "T".into()] },
            Variant { id: "v3".into(), chrom: "chr1".into(), pos: 300, alleles: vec!["A".into(), "T".into()] },
        ],
        genotypes: vec![
            GenotypeObs { id: "gF1".into(), sample_id: "F".into(), variant_id: "v1".into(), calls: vec!["A".into(), "G".into()], likelihood: 0.99, quality: Quality::High, batch: "b1".into() },
            GenotypeObs { id: "gF2".into(), sample_id: "F".into(), variant_id: "v2".into(), calls: vec!["C".into(), "T".into()], likelihood: 0.99, quality: Quality::High, batch: "b1".into() },
            GenotypeObs { id: "gF3".into(), sample_id: "F".into(), variant_id: "v3".into(), calls: vec!["A".into(), "T".into()], likelihood: 0.99, quality: Quality::High, batch: "b1".into() },
            GenotypeObs { id: "gM1".into(), sample_id: "M".into(), variant_id: "v1".into(), calls: vec!["A".into(), "G".into()], likelihood: 0.99, quality: Quality::High, batch: "b1".into() },
            GenotypeObs { id: "gM2".into(), sample_id: "M".into(), variant_id: "v2".into(), calls: vec!["C".into(), "T".into()], likelihood: 0.99, quality: Quality::High, batch: "b1".into() },
            GenotypeObs { id: "gM3".into(), sample_id: "M".into(), variant_id: "v3".into(), calls: vec!["A".into(), "T".into()], likelihood: 0.99, quality: Quality::High, batch: "b1".into() },
            GenotypeObs { id: "gC1".into(), sample_id: "C".into(), variant_id: "v1".into(), calls: vec!["A".into(), "G".into()], likelihood: 0.99, quality: Quality::High, batch: "b1".into() },
            GenotypeObs { id: "gC2".into(), sample_id: "C".into(), variant_id: "v2".into(), calls: vec!["C".into(), "T".into()], likelihood: 0.99, quality: Quality::High, batch: "b1".into() },
            GenotypeObs { id: "gC3".into(), sample_id: "C".into(), variant_id: "v3".into(), calls: vec!["A".into(), "T".into()], likelihood: 0.99, quality: Quality::High, batch: "b1".into() },
        ],
        read_links: vec![
            ReadLinkObs { id: "read:F1".into(), sample_id: "F".into(), left_variant_id: "v1".into(), right_variant_id: "v2".into(), left_allele: "A".into(), right_allele: "C".into(), weight: 2.0, batch: "b1".into() },
            ReadLinkObs { id: "read:F2".into(), sample_id: "F".into(), left_variant_id: "v2".into(), right_variant_id: "v3".into(), left_allele: "T".into(), right_allele: "T".into(), weight: 1.5, batch: "b1".into() },
        ],
    }
}
