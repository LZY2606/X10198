use crate::db;
use crate::models::*;
use rusqlite::Connection;

pub fn seed_demo(conn: &Connection) -> rusqlite::Result<()> {
    let payload = ImportPayload {
        batch_id: Some("synthetic-trio-v1".into()),
        samples: vec![
            SampleInput {
                anon_id: "S-FA".into(),
                label: "匿名父亲".into(),
            },
            SampleInput {
                anon_id: "S-MO".into(),
                label: "匿名母亲".into(),
            },
            SampleInput {
                anon_id: "S-CH".into(),
                label: "匿名孩子".into(),
            },
            SampleInput {
                anon_id: "S-DUP".into(),
                label: "匿名重复样本".into(),
            },
        ],
        variants: vec![
            VariantInput {
                variant_id: "v1".into(),
                chrom: "chr1".into(),
                position: 100,
                reference: "A".into(),
                alternate: "G".into(),
            },
            VariantInput {
                variant_id: "v2".into(),
                chrom: "chr1".into(),
                position: 220,
                reference: "C".into(),
                alternate: "T".into(),
            },
            VariantInput {
                variant_id: "v3".into(),
                chrom: "chr1".into(),
                position: 340,
                reference: "G".into(),
                alternate: "A".into(),
            },
            VariantInput {
                variant_id: "v4".into(),
                chrom: "chr2".into(),
                position: 90,
                reference: "T".into(),
                alternate: "C".into(),
            },
        ],
        relationships: vec![
            RelationshipInput {
                relationship_id: Some("rel-trio".into()),
                child_id: "S-CH".into(),
                father_id: Some("S-FA".into()),
                mother_id: Some("S-MO".into()),
                duplicate_of_id: None,
                kind: "trio".into(),
                confidence: "confirmed".into(),
                source_batch: "synthetic".into(),
            },
            RelationshipInput {
                relationship_id: Some("rel-dup".into()),
                child_id: "S-DUP".into(),
                father_id: None,
                mother_id: None,
                duplicate_of_id: Some("S-CH".into()),
                kind: "duplicate".into(),
                confidence: "confirmed".into(),
                source_batch: "synthetic".into(),
            },
        ],
        genotypes: vec![
            genotype("g-fa-v1", "S-FA", "v1", vec!["A", "G"], "high"),
            genotype("g-mo-v1", "S-MO", "v1", vec!["A", "A"], "high"),
            genotype("g-ch-v1", "S-CH", "v1", vec!["A", "G"], "high"),
            genotype("g-fa-v2", "S-FA", "v2", vec!["C", "T"], "high"),
            genotype("g-mo-v2", "S-MO", "v2", vec!["C", "T"], "high"),
            genotype("g-ch-v2", "S-CH", "v2", vec!["C", "C"], "high"),
            genotype("g-fa-v3", "S-FA", "v3", vec!["G", "G"], "high"),
            genotype("g-mo-v3", "S-MO", "v3", vec!["A", "A"], "high"),
            GenotypeInput {
                observation_id: Some("g-ch-v3".into()),
                sample_id: "S-CH".into(),
                variant_id: "v3".into(),
                alleles: vec!["T".to_string(), "A".to_string()],
                is_missing: false,
                likelihood: 0.61,
                quality: "high".into(),
                batch_id: "synthetic".into(),
            },
            genotype("g-ch-v4", "S-CH", "v4", vec!["T", "C"], "high"),
            genotype("g-dup-v2", "S-DUP", "v2", vec!["T", "T"], "high"),
        ],
        read_links: vec![
            ReadLinkInput {
                link_id: "read-fa-12".into(),
                sample_id: "S-FA".into(),
                variant_a: "v1".into(),
                variant_b: "v2".into(),
                allele_a_index: 1,
                allele_b_index: 1,
                weight: 2.0,
                batch_id: "synthetic".into(),
            },
            ReadLinkInput {
                link_id: "read-mo-12".into(),
                sample_id: "S-MO".into(),
                variant_a: "v1".into(),
                variant_b: "v2".into(),
                allele_a_index: 0,
                allele_b_index: 0,
                weight: 2.0,
                batch_id: "synthetic".into(),
            },
            ReadLinkInput {
                link_id: "read-ch-12".into(),
                sample_id: "S-CH".into(),
                variant_a: "v1".into(),
                variant_b: "v2".into(),
                allele_a_index: 0,
                allele_b_index: 0,
                weight: 2.0,
                batch_id: "synthetic".into(),
            },
        ],
        transmissions: vec![
            TransmissionInput {
                transmission_id: "tx-ch-fa-v1".into(),
                child_id: "S-CH".into(),
                parent_id: "S-FA".into(),
                parent_role: "father".into(),
                variant_id: "v1".into(),
                child_allele_index: 1,
                parent_allele_index: 1,
                weight: 1.0,
                batch_id: "synthetic".into(),
            },
            TransmissionInput {
                transmission_id: "tx-ch-mo-v1".into(),
                child_id: "S-CH".into(),
                parent_id: "S-MO".into(),
                parent_role: "mother".into(),
                variant_id: "v1".into(),
                child_allele_index: 0,
                parent_allele_index: 0,
                weight: 1.0,
                batch_id: "synthetic".into(),
            },
            TransmissionInput {
                transmission_id: "tx-ch-fa-v2".into(),
                child_id: "S-CH".into(),
                parent_id: "S-FA".into(),
                parent_role: "father".into(),
                variant_id: "v2".into(),
                child_allele_index: 0,
                parent_allele_index: 0,
                weight: 1.0,
                batch_id: "synthetic".into(),
            },
            TransmissionInput {
                transmission_id: "tx-ch-mo-v2".into(),
                child_id: "S-CH".into(),
                parent_id: "S-MO".into(),
                parent_role: "mother".into(),
                variant_id: "v2".into(),
                child_allele_index: 0,
                parent_allele_index: 1,
                weight: 1.0,
                batch_id: "synthetic".into(),
            },
        ],
    };
    db::import_payload(conn, &payload)?;
    Ok(())
}

fn genotype(
    id: &str,
    sample: &str,
    variant: &str,
    alleles: Vec<&str>,
    quality: &str,
) -> GenotypeInput {
    GenotypeInput {
        observation_id: Some(id.into()),
        sample_id: sample.into(),
        variant_id: variant.into(),
        alleles: alleles.into_iter().map(str::to_string).collect(),
        is_missing: false,
        likelihood: 0.98,
        quality: quality.into(),
        batch_id: "synthetic".into(),
    }
}
