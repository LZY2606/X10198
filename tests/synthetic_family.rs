use phase_weave::engine::{import_to_input, analyze, stable_hash};
use phase_weave::model::*;
use phase_weave::store::Store;
use std::collections::BTreeMap;

fn sample(id: &str) -> ObservationInput {
    ObservationInput::Sample(SampleInput {
        id: id.into(), anonymous_label: format!("匿名-{id}"), batch: "synthetic".into(),
    })
}
fn variant(id: &str, pos: i64) -> ObservationInput {
    ObservationInput::Variant(VariantInput {
        id: id.into(), chromosome: "chr1".into(), position: pos,
        reference_allele: "A".into(), alternate_alleles: vec!["G".into(), "T".into()],
        batch: "synthetic".into(),
    })
}
fn genotype(id: &str, person: &str, variant_id: &str, alleles: &[&str], quality: f64, missing: bool, low: bool) -> ObservationInput {
    ObservationInput::Genotype(GenotypeInput {
        id: id.into(), sample: person.into(), variant: variant_id.into(),
        alleles: alleles.iter().map(|value| value.to_string()).collect(),
        likelihoods: BTreeMap::new(), quality, missing,
        low_quality_heterozygous: low, duplicate_observation: false, batch: "synthetic".into(),
    })
}
fn link(id: &str, person: &str, left: &str, right: &str, la: &str, ra: &str) -> ObservationInput {
    ObservationInput::ReadLink(ReadLinkInput {
        id: id.into(), sample: person.into(), chromosome: "chr1".into(),
        left_variant: left.into(), right_variant: right.into(),
        left_allele: la.into(), right_allele: ra.into(), weight: 1.0,
        withdrawn: false, batch: "synthetic".into(),
    })
}
fn relation(id: &str, child: &str, father: &str, mother: &str, confidence: f64) -> ObservationInput {
    ObservationInput::Relationship(RelationshipInput {
        id: id.into(), child: child.into(), father: Some(father.into()),
        mother: Some(mother.into()), confidence, status: None, batch: None,
    })
}
fn payload(version: &str, budget: Option<usize>, observations: Vec<ObservationInput>) -> ImportPayload {
    ImportPayload { version_id: Some(version.into()), note: None, candidate_budget: budget, observations }
}
fn run(observations: Vec<ObservationInput>, budget: Option<usize>) -> phase_weave::engine::Analysis {
    let (input, conflicts) = import_to_input(payload("test-version", budget, observations));
    analyze(input, DecisionOverlay::default(), conflicts)
}

#[test]
fn paternal_maternal_swap_remains_equivalent_without_phase_link() {
    let result = run(vec![
        sample("f"), sample("m"), sample("c"), relation("r", "c", "f", "m", 1.0),
        variant("v", 1), genotype("gf", "f", "v", &["A", "G"], 90.0, false, false),
        genotype("gm", "m", "v", &["A", "G"], 90.0, false, false),
        genotype("gc", "c", "v", &["A", "G"], 90.0, false, false),
    ], Some(16));
    let candidates = &result.candidates;
    assert!(candidates.len() >= 2);
    let best = candidates.iter().map(|c| c.score).fold(f64::MIN, f64::max);
    let tied: Vec<_> = candidates.iter().filter(|c| (c.score - best).abs() < 1e-9).collect();
    assert!(tied.len() >= 2, "父源/母源互换必须同分并列: {tied:?}");
    let sources: std::collections::BTreeSet<_> = tied.iter().flat_map(|c| c.relation_sources.clone()).map(|s| (s.paternal_allele, s.maternal_allele)).collect();
    assert!(sources.contains(&("A".to_string(), "G".to_string())));
    assert!(sources.contains(&("G".to_string(), "A".to_string())));
}

#[test]
fn triallelic_and_missing_are_isolated_or_warned_not_deleted() {
    let result = run(vec![
        sample("f"), sample("m"), sample("c"), relation("r", "c", "f", "m", 1.0),
        variant("v1", 1), variant("v2", 2),
        genotype("gf1", "f", "v1", &["A", "A"], 90.0, false, false),
        genotype("gm1", "m", "v1", &["A", "A"], 90.0, false, false),
        genotype("gc1", "c", "v1", &["A", "G", "T"], 90.0, false, false),
        genotype("gf2", "f", "v2", &["A", "A"], 90.0, false, false),
        genotype("gm2", "m", "v2", &["A", "A"], 90.0, false, false),
        genotype("gc2", "c", "v2", &[], 0.0, true, false),
    ], Some(16));
    assert!(result.conflicts.iter().any(|c| c.kind == "triallelic_error" && c.observations.contains(&"gc1".to_string())));
    assert!(result.warnings.iter().any(|w| w.kind == "missing_genotype" && w.observations.contains(&"gc2".to_string())));
    assert!(result.input.genotypes.iter().any(|g| g.id == "gc1"));
}

#[test]
fn de_novo_is_a_review_candidate_with_minimal_repair() {
    let result = run(vec![
        sample("f"), sample("m"), sample("c"), relation("r", "c", "f", "m", 1.0),
        variant("v", 1), genotype("gf", "f", "v", &["A", "A"], 90.0, false, false),
        genotype("gm", "m", "v", &["A", "A"], 90.0, false, false),
        genotype("gc", "c", "v", &["A", "G"], 90.0, false, false),
    ], Some(16));
    assert!(result.conflicts.iter().any(|c| c.kind == "de_novo_candidate"));
    assert!(result.candidates.iter().flat_map(|c| c.de_novo.clone()).any(|d| d.allele == "G"));
}

#[test]
fn read_link_merges_block_and_withdrawal_splits_only_affected_region() {
    let observations = vec![
        sample("c"), variant("v1", 1), variant("v2", 2), variant("v3", 3),
        genotype("g1", "c", "v1", &["A", "G"], 90.0, false, false),
        genotype("g2", "c", "v2", &["A", "G"], 90.0, false, false),
        genotype("g3", "c", "v3", &["A", "G"], 90.0, false, false),
        link("l12", "c", "v1", "v2", "G", "G"),
        link("l23", "c", "v2", "v3", "G", "G"),
    ];
    let merged = run(observations.clone(), Some(16));
    assert_eq!(merged.blocks.len(), 1);
    assert!(merged.bridge_events.iter().any(|e| e.kind == "merge" && e.observation_id == "l12"));

    let (input, conflicts) = import_to_input(payload("split", Some(16), observations));
    let mut overlay = DecisionOverlay::default();
    overlay.decisions.push(Decision::WithdrawReadLink { read_link_id: "l12".into(), reason: None });
    let split = analyze(input, overlay, conflicts);
    assert_eq!(split.blocks.len(), 2);
    assert!(split.bridge_events.iter().any(|e| e.kind == "withdraw_split_region" && e.observation_id == "l12"));
}

#[test]
fn budget_exhaustion_is_reported_and_not_disguised_as_unique() {
    let result = run(vec![
        sample("f"), sample("m"), sample("c"), relation("r", "c", "f", "m", 1.0),
        variant("v", 1), genotype("gf", "f", "v", &["A", "G"], 90.0, false, false),
        genotype("gm", "m", "v", &["A", "G"], 90.0, false, false),
        genotype("gc", "c", "v", &["A", "G"], 90.0, false, false),
    ], Some(1));
    assert!(result.budget_exhausted);
    assert!(result.candidates.iter().any(|c| c.budget_limited || c.tied));
}

#[test]
fn store_replays_rollback_revises_relationship_and_keeps_stale_concurrent_verdicts() {
    let store = Store::in_memory().unwrap();
    let mut first = vec![
        sample("f"), sample("m"), sample("c"), relation("r", "c", "f", "m", 1.0),
        variant("v", 1), genotype("gf", "f", "v", &["A", "A"], 90.0, false, false),
        genotype("gm", "m", "v", &["A", "A"], 90.0, false, false),
        genotype("gc", "c", "v", &["A", "G"], 90.0, false, false),
    ];
    let first_version = format!("store-v1-{}", stable_hash("first"));
    store.import(payload(&first_version, Some(8), first.clone())).unwrap();
    let first_branch = store.current_branch_id().unwrap();
    store.record_decision(Decision::MarkRelationshipUncertain { relationship_id: "r".into(), note: Some("待确认".into()) }).unwrap();
    store.rollback_current().unwrap();
    let rolled_back = store.current_analysis().unwrap().1;
    assert!(rolled_back.overlay.uncertain_relationships.is_empty());

    first.push(sample("x"));
    let second_version = format!("store-v2-{}", stable_hash("second"));
    store.import(payload(&second_version, Some(8), first)).unwrap();
    assert_ne!(store.current_branch_id().unwrap(), first_branch);
    store.set_current_branch(&first_branch).unwrap();
    store.record_decision(Decision::LockPhase {
        block_id: rolled_back.blocks[0].id.clone(), variant_id: "v".into(),
        allele_a: "A".into(), allele_b: "G".into(),
    }).unwrap();
    store.set_current_branch(&store.branches().unwrap().iter().find(|b| b.version_id == second_version).unwrap().id.clone()).unwrap();
    let current = store.current_analysis().unwrap().1;
    assert!(!current.overlay.locked_phases.iter().any(|(_, lock)| lock.variant_id == "v"));
    let export = store.export_current().unwrap();
    assert!(export.stale_events.iter().any(|event| event.version_id == first_version));
    assert_eq!(export.pinned_version_id, second_version);

    let _ = store.record_decision(Decision::ReviseRelationship {
        relationship_id: "r".into(), father: Some("f".into()), mother: None,
        confidence: 0.4, note: Some("母亲身份修订".into()),
    }).unwrap();
    let revised = store.current_analysis().unwrap().1;
    assert!(revised.warnings.iter().any(|w| w.kind == "parent_identity_uncertain"));
}
