use phase_weaver::db::{self, Db};
use phase_weaver::engine;
use phase_weaver::models::*;
use serde_json::json;

fn memory() -> Db {
    let mut db = Db::open(":memory:").unwrap();
    db.tx(db::ensure_branches).unwrap();
    db
}

fn import(db: &mut Db, payload: ImportPayload) -> i64 {
    db.tx(|conn| db::import_payload(conn, &payload).map(|(version, _)| version))
        .unwrap()
}

fn state(db: &Db, budget: usize) -> engine::State {
    engine::build_state(&db.0, "main", budget).unwrap()
}

fn sample(id: &str) -> SampleInput {
    SampleInput {
        anon_id: id.into(),
        label: format!("匿名{id}"),
    }
}

fn variant(id: &str, chrom: &str, position: i64) -> VariantInput {
    VariantInput {
        variant_id: id.into(),
        chrom: chrom.into(),
        position,
        reference: "A".into(),
        alternate: "G".into(),
    }
}

fn trio(child: &str, father: &str, mother: &str) -> RelationshipInput {
    RelationshipInput {
        relationship_id: Some(format!("rel-{child}")),
        child_id: child.into(),
        father_id: Some(father.into()),
        mother_id: Some(mother.into()),
        duplicate_of_id: None,
        kind: "trio".into(),
        confidence: "confirmed".into(),
        source_batch: "test".into(),
    }
}

fn geno(
    id: &str,
    sample_id: &str,
    variant_id: &str,
    alleles: Vec<&str>,
    quality: &str,
    missing: bool,
) -> GenotypeInput {
    GenotypeInput {
        observation_id: Some(id.into()),
        sample_id: sample_id.into(),
        variant_id: variant_id.into(),
        alleles: alleles.into_iter().map(str::to_string).collect(),
        is_missing: missing,
        likelihood: 0.95,
        quality: quality.into(),
        batch_id: "test".into(),
    }
}

fn g(id: &str, sample_id: &str, variant_id: &str, alleles: Vec<&str>) -> GenotypeInput {
    geno(id, sample_id, variant_id, alleles, "high", false)
}

fn link(id: &str, sample_id: &str, a: &str, b: &str, ai: usize, bi: usize) -> ReadLinkInput {
    ReadLinkInput {
        link_id: id.into(),
        sample_id: sample_id.into(),
        variant_a: a.into(),
        variant_b: b.into(),
        allele_a_index: ai,
        allele_b_index: bi,
        weight: 1.0,
        batch_id: "test".into(),
    }
}

fn decision(db: &mut Db, kind: &str, payload: serde_json::Value, version: Option<i64>) -> i64 {
    db.tx(|conn| db::append_event(conn, "main", kind, &payload, version))
        .unwrap()
}

#[test]
fn parent_origin_swap_equivalence_without_anchor_keeps_ties() {
    let mut db = memory();
    import(
        &mut db,
        ImportPayload {
            batch_id: Some("swap".into()),
            samples: vec![sample("C")],
            variants: vec![variant("v1", "chr1", 1), variant("v2", "chr1", 2)],
            relationships: vec![],
            genotypes: vec![
                g("c1", "C", "v1", vec!["A", "G"]),
                g("c2", "C", "v2", vec!["A", "G"]),
            ],
            read_links: vec![link("l1", "C", "v1", "v2", 0, 0)],
            transmissions: vec![],
        },
    );
    let s = state(&db, 32);
    let block = &s.blocks[0];
    assert_eq!(block.candidates.len(), 2);
    assert!(block.candidates.iter().all(|c| c.label_swap_equivalent));
    assert_eq!(block.candidates[0].score, block.candidates[1].score);
}

#[test]
fn triallelic_error_is_isolated_with_minimal_repairs() {
    let mut db = memory();
    import(
        &mut db,
        ImportPayload {
            batch_id: Some("tri".into()),
            samples: vec![sample("F"), sample("M"), sample("C")],
            variants: vec![variant("v1", "chr1", 1)],
            relationships: vec![trio("C", "F", "M")],
            genotypes: vec![
                g("f", "F", "v1", vec!["A", "A"]),
                g("m", "M", "v1", vec!["G", "G"]),
                geno("c", "C", "v1", vec!["T", "G"], "high", false),
            ],
            read_links: vec![],
            transmissions: vec![],
        },
    );
    let s = state(&db, 32);
    let conflict = s
        .conflicts
        .iter()
        .find(|c| c.kind == "triallelic_error")
        .unwrap();
    assert_eq!(conflict.minimal_repair_sets, vec![vec!["c".to_string()]]);
    assert!(s
        .ignored_observations
        .iter()
        .any(|x| x.observation_id == "c" && x.kind == "triallelic_call"));
}

#[test]
fn missing_genotype_is_evidence_not_deleted() {
    let mut db = memory();
    import(
        &mut db,
        ImportPayload {
            batch_id: Some("missing".into()),
            samples: vec![sample("F"), sample("M"), sample("C")],
            variants: vec![variant("v1", "chr1", 1)],
            relationships: vec![trio("C", "F", "M")],
            genotypes: vec![
                g("f", "F", "v1", vec!["A", "A"]),
                g("m", "M", "v1", vec!["G", "G"]),
                geno("c", "C", "v1", vec![], "high", true),
            ],
            read_links: vec![],
            transmissions: vec![],
        },
    );
    let s = state(&db, 32);
    assert!(s.conflicts.iter().any(|c| c.kind == "missing_genotype"));
    assert!(s
        .ignored_observations
        .iter()
        .any(|x| x.observation_id == "c"));
}

#[test]
fn de_novo_candidate_is_reported() {
    let mut db = memory();
    import(
        &mut db,
        ImportPayload {
            batch_id: Some("denovo".into()),
            samples: vec![sample("F"), sample("M"), sample("C")],
            variants: vec![variant("v1", "chr1", 1)],
            relationships: vec![trio("C", "F", "M")],
            genotypes: vec![
                g("f", "F", "v1", vec!["A", "A"]),
                g("m", "M", "v1", vec!["A", "A"]),
                g("c", "C", "v1", vec!["A", "G"]),
            ],
            read_links: vec![],
            transmissions: vec![],
        },
    );
    let s = state(&db, 32);
    let conflict = s
        .conflicts
        .iter()
        .find(|c| c.kind == "de_novo_candidate")
        .unwrap();
    assert!(conflict
        .minimal_repair_sets
        .iter()
        .any(|set| set == &vec!["c".to_string()]));
}

#[test]
fn tied_candidates_are_all_retained() {
    let mut db = memory();
    import(
        &mut db,
        ImportPayload {
            batch_id: Some("ties".into()),
            samples: vec![sample("C")],
            variants: vec![variant("v1", "chr1", 1)],
            relationships: vec![],
            genotypes: vec![g("c", "C", "v1", vec!["A", "G"])],
            read_links: vec![],
            transmissions: vec![],
        },
    );
    let s = state(&db, 32);
    assert_eq!(s.blocks[0].candidates.len(), 2);
    assert!(s.blocks[0].no_unique_solution);
}

#[test]
fn withdrawing_bridge_only_splits_affected_region() {
    let mut db = memory();
    import(
        &mut db,
        ImportPayload {
            batch_id: Some("bridge".into()),
            samples: vec![sample("C")],
            variants: vec![
                variant("v1", "chr1", 1),
                variant("v2", "chr1", 2),
                variant("v3", "chr1", 3),
            ],
            relationships: vec![],
            genotypes: vec![
                g("c1", "C", "v1", vec!["A", "G"]),
                g("c2", "C", "v2", vec!["A", "G"]),
                g("c3", "C", "v3", vec!["A", "G"]),
            ],
            read_links: vec![
                link("bridge12", "C", "v1", "v2", 0, 0),
                link("keep23", "C", "v2", "v3", 0, 0),
            ],
            transmissions: vec![],
        },
    );
    assert_eq!(state(&db, 32).blocks.len(), 1);
    decision(
        &mut db,
        "withdraw_read_link",
        json!({"read_link_id":"bridge12"}),
        Some(1),
    );
    let s = state(&db, 32);
    assert_eq!(s.blocks.len(), 2);
    assert!(s.withdrawn_read_links.contains(&"bridge12".to_string()));
}

#[test]
fn relationship_revision_removes_mendelian_conflict() {
    let mut db = memory();
    import(
        &mut db,
        ImportPayload {
            batch_id: Some("rev".into()),
            samples: vec![sample("F"), sample("M"), sample("C"), sample("B")],
            variants: vec![variant("v1", "chr1", 1)],
            relationships: vec![trio("C", "F", "M")],
            genotypes: vec![
                g("f", "F", "v1", vec!["A", "A"]),
                g("m", "M", "v1", vec!["A", "A"]),
                g("c", "C", "v1", vec!["A", "G"]),
                g("b", "B", "v1", vec!["A", "G"]),
            ],
            read_links: vec![],
            transmissions: vec![],
        },
    );
    assert!(state(&db, 32)
        .conflicts
        .iter()
        .any(|c| c.severity == "hard"));
    decision(
        &mut db,
        "revise_relationship",
        json!({"relationship_id":"rel-C","mother_id":"B","confidence":"confirmed"}),
        Some(1),
    );
    let s = state(&db, 32);
    assert!(!s.conflicts.iter().any(|c| c.severity == "hard"));
}

#[test]
fn old_version_concurrent_adjudication_is_marked_stale() {
    let mut db = memory();
    import(
        &mut db,
        ImportPayload {
            batch_id: Some("v1".into()),
            samples: vec![sample("C")],
            variants: vec![variant("v1", "chr1", 1)],
            relationships: vec![],
            genotypes: vec![g("c", "C", "v1", vec!["A", "G"])],
            read_links: vec![],
            transmissions: vec![],
        },
    );
    import(
        &mut db,
        ImportPayload {
            batch_id: Some("v2".into()),
            samples: vec![sample("C")],
            variants: vec![variant("v1", "chr1", 1)],
            relationships: vec![],
            genotypes: vec![g("c", "C", "v1", vec!["A", "A"])],
            read_links: vec![],
            transmissions: vec![],
        },
    );
    decision(
        &mut db,
        "lock_phase",
        json!({"sample_id":"C","variant_id":"v1","haplotype_a_origin":"father"}),
        Some(1),
    );
    let s = state(&db, 32);
    assert!(s.decisions.iter().any(|d| d.stale
        && d.stale_reason
            .as_deref()
            .unwrap()
            .contains("当前有效版本为 2")));
}

#[test]
fn budget_exhaustion_is_not_reported_as_unique() {
    let mut db = memory();
    import(
        &mut db,
        ImportPayload {
            batch_id: Some("budget".into()),
            samples: vec![sample("C")],
            variants: vec![variant("v1", "chr1", 1)],
            relationships: vec![],
            genotypes: vec![g("c", "C", "v1", vec!["A", "G"])],
            read_links: vec![],
            transmissions: vec![],
        },
    );
    let s = state(&db, 1);
    assert!(s.blocks[0].budget_reached);
    assert!(s.blocks[0].no_unique_solution || s.blocks[0].search_incomplete);
}

#[test]
fn rollback_reactivates_block_candidate_and_export_replays() {
    let mut db = memory();
    import(
        &mut db,
        ImportPayload {
            batch_id: Some("rollback".into()),
            samples: vec![sample("C")],
            variants: vec![variant("v1", "chr1", 1)],
            relationships: vec![],
            genotypes: vec![g("c", "C", "v1", vec!["A", "G"])],
            read_links: vec![],
            transmissions: vec![],
        },
    );
    let signature = state(&db, 32).blocks[0].candidates[0].signature.clone();
    let event = decision(
        &mut db,
        "accept_candidate",
        json!({"block_id":"block:C:v1","candidate_signature":signature}),
        Some(1),
    );
    assert!(state(&db, 32).blocks[0]
        .candidates
        .iter()
        .any(|c| c.accepted));
    decision(
        &mut db,
        "rollback_decision",
        json!({"event_id":event}),
        None,
    );
    let s = state(&db, 32);
    assert!(!s.blocks[0].candidates.iter().any(|c| c.accepted));
    let export = engine::export_state(&s);
    assert_eq!(export["pinned_input_version"], serde_json::Value::Null);
    assert!(export["replay_steps"].is_array());
}

#[test]
fn cross_chromosome_observations_do_not_merge_blocks() {
    let mut db = memory();
    import(
        &mut db,
        ImportPayload {
            batch_id: Some("cross".into()),
            samples: vec![sample("C")],
            variants: vec![variant("v1", "chr1", 1), variant("v2", "chr2", 2)],
            relationships: vec![],
            genotypes: vec![
                g("c1", "C", "v1", vec!["A", "G"]),
                g("c2", "C", "v2", vec!["A", "G"]),
            ],
            read_links: vec![link("bad-read", "C", "v1", "v2", 1, 1)],
            transmissions: vec![],
        },
    );
    let s = state(&db, 32);
    assert_eq!(s.blocks.len(), 2);
    assert!(s
        .ignored_observations
        .iter()
        .any(|x| x.kind == "read_link_cross_chromosome"));
}

#[test]
fn pinning_old_version_replays_original_observations() {
    let mut db = memory();
    import(
        &mut db,
        ImportPayload {
            batch_id: Some("v1".into()),
            samples: vec![sample("C")],
            variants: vec![variant("v1", "chr1", 1)],
            relationships: vec![],
            genotypes: vec![g("c", "C", "v1", vec!["A", "G"])],
            read_links: vec![],
            transmissions: vec![],
        },
    );
    import(
        &mut db,
        ImportPayload {
            batch_id: Some("v2".into()),
            samples: vec![sample("C")],
            variants: vec![variant("v1", "chr1", 1)],
            relationships: vec![],
            genotypes: vec![g("c", "C", "v1", vec!["A", "A"])],
            read_links: vec![],
            transmissions: vec![],
        },
    );
    assert_eq!(state(&db, 32).blocks.len(), 0);
    decision(&mut db, "pin_version", json!({"version": 1}), None);
    let pinned = state(&db, 32);
    assert_eq!(pinned.effective_version, 1);
    assert_eq!(pinned.blocks.len(), 1);
    assert_eq!(
        pinned.genotypes[0].alleles,
        vec!["A".to_string(), "G".to_string()]
    );
}
