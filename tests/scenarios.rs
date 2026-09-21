mod common;

use common::*;
use phase_weave::model::*;


#[test]
fn parent_origin_swap_is_equivalent_single_candidate() {
    let mut fx = Fixture::new();
    fx.import(&trio_bundle());
    let view = fx.view("main");
    let block = find_block(&view, 3, 100).expect("child should have a phased block");
    // Fixing the first variant's orientation quotients out the global
    // paternal/maternal haplotype swap: exactly one equivalence class.
    assert_eq!(block.candidates.len(), 1, "swap-equivalent candidates must collapse: {:?}",
        block.candidates);
    let c = &block.candidates[0];
    assert!((c.score - 1.0).abs() < 1e-9);
    assert_eq!(c.orientation, vec![true, true]);
}

#[test]
fn triallelic_error_is_isolated_with_minimal_set() {
    let mut fx = Fixture::new();
    let mut b = trio_bundle();
    // Third site: father T/T (idx 5), mother T/T, child A/A (idx 0).
    b.variants.push(var(103, 300, vec!["A", "G", "T"]));
    b.observations.push(obs(207, 1, 103, triallelic_gls(gt(2, 2), 90.0), 90.0));
    // Mother carries one A allele: the child genotype is not a de novo but a
    // plain Mendelian inconsistency at a triallelic site.
    b.observations.push(obs(208, 2, 103, triallelic_gls(gt(0, 2), 90.0), 90.0));
    b.observations.push(obs(209, 3, 103, triallelic_gls(gt(0, 0), 60.0), 60.0));
    fx.import(&b);
    let view = fx.view("main");
    let c = view
        .conflicts
        .iter()
        .find(|c| c.variant == Some(103))
        .expect("triallelic site must produce a conflict");
    assert_eq!(c.kind, ConflictKind::MendelianInconsistent);
    assert_eq!(c.minimal_ignore, vec![209],
        "the low-quality child observation is the minimal quarantine set");
    // The inconsistent row is isolated as evidence, never deleted.
    assert!(c.evidence.contains(&207) && c.evidence.contains(&208) && c.evidence.contains(&209));
    assert!(view.matrix[&3][&103].ignored);
    assert!(view.matrix[&3][&103].genotype.contains('A'));
}

#[test]
fn missing_genotype_is_a_conflict_not_a_hard_failure() {
    let mut fx = Fixture::new();
    let mut b = trio_bundle();
    // Mother missing at site 2: empty GLs.
    let mut missing = obs(301, 2, 102, vec![], 0.0);
    missing.version = 2;
    b.observations.push(missing);
    fx.import(&b);
    let view = fx.view("main");
    assert!(view.conflicts.iter().any(|c| c.kind == ConflictKind::MissingGenotype
        && c.sample == Some(2) && c.variant == Some(102)));
    // Father is still hom at site 102, so transmission phasing still works.
    let block = find_block(&view, 3, 100).expect("block survives missing parent site");
    assert_eq!(block.candidates.len(), 1);
}

#[test]
fn de_novo_candidate_flagged() {
    let mut fx = Fixture::new();
    let mut b = trio_bundle();
    // de novo: both parents hom ref, child hom alt at a new biallelic site.
    b.variants.push(var(104, 400, vec!["C", "A"]));
    b.observations.push(obs(311, 1, 104, biallelic_gls(0, 99.0), 99.0));
    b.observations.push(obs(312, 2, 104, biallelic_gls(0, 99.0), 99.0));
    b.observations.push(obs(313, 3, 104, biallelic_gls(2, 99.0), 99.0));
    fx.import(&b);
    let view = fx.view("main");
    let c = view.conflicts.iter().find(|c| c.variant == Some(104)).unwrap();
    assert_eq!(c.kind, ConflictKind::DeNovoCandidate);
    assert!(!c.minimal_ignore.is_empty());
}

#[test]
fn tied_candidates_all_retained() {
    let mut fx = Fixture::new();
    // Two unrelated het sites in the child linked by contradictory read links
    // of equal weight -> both orientations tie.
    let b = ImportBundle {
        samples: vec![sample(3, "S-C")],
        relationships: vec![],
        variants: vec![var(101, 100, vec!["A", "G"]), var(102, 200, vec!["C", "T"])],
        observations: vec![
            obs(205, 3, 101, biallelic_gls(1, 95.0), 95.0),
            obs(206, 3, 102, biallelic_gls(1, 95.0), 95.0),
        ],
        read_links: vec![
            ReadLink {
                id: 401, sample: 3, var_a: 101, var_b: 102, same_haplotype: true,
                weight: 1.0, retracted: false, batch: "b1".into(), version: 1,
            },
            ReadLink {
                id: 402, sample: 3, var_a: 101, var_b: 102, same_haplotype: false,
                weight: 1.0, retracted: false, batch: "b1".into(), version: 1,
            },
        ],
    };
    fx.import(&b);
    let view = fx.view("main");
    let block = find_block(&view, 3, 100).unwrap();
    assert_eq!(block.candidates.len(), 2, "equal-score candidates are both kept");
    assert_eq!(block.candidates[0].score, block.candidates[1].score);
}

fn bridge_bundle() -> ImportBundle {
    // Child het at 4 sites. The father is hom at sites 100,200 (one
    // transmission pair) and heterozygous at 300,400 (uninformative, no
    // transmission edges); read links join 300-400 and bridge 200-300.
    ImportBundle {
        samples: vec![sample(1, "S-F"), sample(3, "S-C")],
        relationships: vec![rel(11, 3, 1, ParentKind::Father)],
        variants: vec![
            var(101, 100, vec!["A", "G"]), var(102, 200, vec!["C", "T"]),
            var(103, 300, vec!["G", "T"]), var(104, 400, vec!["A", "C"]),
        ],
        observations: vec![
            obs(201, 1, 101, biallelic_gls(0, 99.0), 99.0),
            obs(202, 1, 102, biallelic_gls(0, 99.0), 99.0),
            obs(203, 1, 103, biallelic_gls(1, 99.0), 99.0),
            obs(204, 1, 104, biallelic_gls(1, 99.0), 99.0),
            obs(205, 3, 101, biallelic_gls(1, 95.0), 95.0),
            obs(206, 3, 102, biallelic_gls(1, 95.0), 95.0),
            obs(207, 3, 103, biallelic_gls(1, 95.0), 95.0),
            obs(208, 3, 104, biallelic_gls(1, 95.0), 95.0),
        ],
        read_links: vec![
            ReadLink {
                id: 400, sample: 3, var_a: 103, var_b: 104, same_haplotype: true,
                weight: 2.0, retracted: false, batch: "b1".into(), version: 1,
            },
            ReadLink {
                id: 401, sample: 3, var_a: 102, var_b: 103, same_haplotype: true,
                weight: 2.0, retracted: false, batch: "b1".into(), version: 1,
            },
        ],
    }
}

#[test]
fn bridge_merge_records_evidence_and_retraction_splits_only_affected_region() {
    let mut fx = Fixture::new();
    fx.import(&bridge_bundle());
    let v1 = fx.view("main");
    let merged = find_block(&v1, 3, 100).expect("bridge merges into one block");
    assert_eq!(merged.variant_ids, vec![101, 102, 103, 104]);
    assert_eq!(merged.bridge_evidence, vec![401]);

    // Retract the bridge read link: only the bridged region splits.
    fx.decide(1, Decision::RetractReadLink { link_id: 401 });
    let v2 = fx.view("main");
    let starts: Vec<i64> = v2.blocks.iter().map(|b| b.start_pos).collect();
    assert_eq!(starts, vec![100, 300]);
    let left = find_block(&v2, 3, 100).unwrap();
    let right = find_block(&v2, 3, 300).unwrap();
    assert_eq!(left.variant_ids, vec![101, 102]);
    assert_eq!(right.variant_ids, vec![103, 104]);
    assert!(left.bridge_evidence.is_empty() && right.bridge_evidence.is_empty());
}

#[test]
fn relationship_revision_removes_and_restores_transmission_evidence() {
    let mut fx = Fixture::new();
    fx.import(&trio_bundle());
    // Mark father relationship to-confirm: transmission evidence is excluded,
    // the child block disappears and the conflict is shown.
    fx.decide(1, Decision::MarkRelationship {
        relationship_id: 11, status: RelStatus::ToConfirm,
    });
    let v1 = fx.view("main");
    assert!(find_block(&v1, 3, 100).is_none());
    assert!(v1.conflicts.iter().any(|c| c.kind == ConflictKind::UncertainParentage));
    // Rollback restores the relationship and the block.
    fx.store.rollback("main").unwrap();
    let v2 = fx.view("main");
    assert!(find_block(&v2, 3, 100).is_some());
}

#[test]
fn stale_version_concurrent_adjudication_rejected() {
    use phase_weave::App;
    let tmp = std::env::temp_dir().join(format!("pw-stale-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    let app = App::open(tmp.to_str().unwrap()).unwrap();
    app.import(&trio_bundle()); // version 1
    // Reviewer holds version 1; a new batch imports version 2.
    app.import(&ImportBundle {
        samples: vec![], relationships: vec![], variants: vec![],
        observations: vec![], read_links: vec![],
    });
    let err = app
        .decide("main", 1, Decision::MarkRelationship {
            relationship_id: 11, status: RelStatus::ToConfirm,
        })
        .expect_err("decision based on an old input version must be rejected");
    assert!(matches!(err, phase_weave::AppError::VersionMismatch { .. }));
    // Refreshing to version 2 succeeds.
    app.decide("main", 2, Decision::MarkRelationship {
        relationship_id: 11, status: RelStatus::ToConfirm,
    }).unwrap();
}

#[test]
fn candidate_budget_hit_is_surfaced_not_pretended_unique() {
    let mut fx = Fixture::new();
    // 8 het sites joined only by equal-weight contradictory links: the search
    // space (128 after swap quotient) exceeds the exact cap (64), beam search
    // must flag budget exhaustion.
    let n = 8usize;
    let mut variants = Vec::new();
    let mut observations = Vec::new();
    let mut read_links = Vec::new();
    for i in 0..n {
        let vid = (200 + i) as i64;
        variants.push(var(vid, (100 * (i + 1)) as i64, vec!["A", "G"]));
        observations.push(obs((500 + i) as i64, 3, vid, biallelic_gls(1, 95.0), 95.0));
        if i + 1 < n {
            read_links.push(ReadLink {
                id: (600 + 2 * i) as i64, sample: 3, var_a: vid, var_b: vid + 1,
                same_haplotype: true, weight: 1.0, retracted: false,
                batch: "b1".into(), version: 1,
            });
            read_links.push(ReadLink {
                id: (600 + 2 * i + 1) as i64, sample: 3, var_a: vid, var_b: vid + 1,
                same_haplotype: false, weight: 1.0, retracted: false,
                batch: "b1".into(), version: 1,
            });
        }
    }
    let b = ImportBundle {
        samples: vec![sample(3, "S-C")],
        relationships: vec![],
        variants,
        observations,
        read_links,
    };
    fx.import(&b);
    let view = fx.view("main");
    let block = find_block(&view, 3, 100).unwrap();
    assert!(block.budget_hit, "over-budget search must be flagged");
    assert!(block.candidates.len() <= phase_weave::engine::CANDIDATE_BUDGET);
}

#[test]
fn locked_local_phase_is_respected_and_rollback_clears_it() {
    let mut fx = Fixture::new();
    fx.import(&trio_bundle());
    fx.decide(1, Decision::LockPhase {
        sample: 3, chrom: "chr1".into(), from_pos: 100, to_pos: 200,
        orientation: vec![(101, true), (102, false)],
    });
    let v1 = fx.view("main");
    let block = find_block(&v1, 3, 100).unwrap();
    // The locked hard edge forces the opposite orientation.
    assert_eq!(block.candidates[0].orientation, vec![true, false]);
    fx.store.rollback("main").unwrap();
    let v2 = fx.view("main");
    let block = find_block(&v2, 3, 100).unwrap();
    assert_eq!(block.candidates[0].orientation, vec![true, true]);
}

#[test]
fn export_pins_versions_and_replay_reproduces_state() {
    use phase_weave::App;
    let tmp = std::env::temp_dir().join(format!("pw-replay-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    let app = App::open(tmp.to_str().unwrap()).unwrap();
    app.import(&trio_bundle());
    app.decide("main", 1, Decision::AcceptCandidate {
        block_id: "B-3-chr1-101".into(), candidate_index: 0,
    }).unwrap();
    let snapshot = app.export("main");
    assert_eq!(snapshot.input_version, 1);
    assert_eq!(snapshot.events.len(), 1);
    assert!(snapshot.pinned_batches.contains(&"b1".to_string()));

    // Rebuild from scratch and replay the pinned events.
    let tmp2 = std::env::temp_dir().join(format!("pw-replay2-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&tmp2);
    let app2 = App::open(tmp2.to_str().unwrap()).unwrap();
    App::replay(&app2, &snapshot, &trio_bundle());
    let view = app2.view("main");
    let block = find_block(&view, 3, 100).unwrap();
    assert!(block.candidates[0].accepted);
}

#[test]
fn duplicate_sample_label_conflict() {
    let mut fx = Fixture::new();
    let mut b = trio_bundle();
    b.samples.push(sample(9, "S-F"));
    fx.import(&b);
    let view = fx.view("main");
    assert!(view.conflicts.iter().any(|c| c.kind == ConflictKind::DuplicateSample));
}

