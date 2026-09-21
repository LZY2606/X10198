#![allow(dead_code)]
use phase_weave::engine::{compute, EngineInput, StateView};
use phase_weave::model::*;
use phase_weave::store::Store;

pub fn biallelic_gls(call: usize, q: f64) -> Vec<f64> {
    // Genotype order 0/0, 0/1, 1/1.
    let strong = (1.0 - 2.0 * 10f64.powf(-q / 10.0)).max(0.0);
    let weak = (1.0 - strong) / 2.0;
    let mut v = vec![weak; 3];
    v[call] = strong;
    v
}

/// GLs over the six unordered genotypes of three alleles (A,G,T).
pub fn triallelic_gls(call: usize, q: f64) -> Vec<f64> {
    let strong = (1.0 - 5.0 * 10f64.powf(-q / 10.0)).max(0.0);
    let weak = (1.0 - strong) / 5.0;
    let mut v = vec![weak; 6];
    v[call] = strong;
    v
}

/// genotype index for unordered allele pair (i<=j)
pub fn gt(i: usize, j: usize) -> usize {
    let (i, j) = (i.min(j), i.max(j));
    j * (j + 1) / 2 + i
}

pub struct Fixture {
    pub store: Store,
}

impl Fixture {
    pub fn new() -> Self {
        Fixture { store: Store::open(":memory:").unwrap() }
    }

    pub fn import(&mut self, b: &ImportBundle) -> i64 {
        self.store.import(b)
    }

    pub fn view(&self, branch: &str) -> StateView {
        compute(&EngineInput {
            samples: self.store.samples(),
            relationships: self.store.relationships(),
            variants: self.store.variants(),
            observations: self.store.observations(),
            read_links: self.store.read_links(),
            events: self.store.events(branch),
            input_version: self.store.input_version(),
        })
    }

    pub fn decide(&self, version: i64, d: Decision) -> i64 {
        self.store.append_event("main", version, &d)
    }
}

pub fn sample(id: i64, label: &str) -> Sample {
    Sample { id, anon_label: label.to_string(), batch: "b1".to_string() }
}

pub fn var(id: i64, pos: i64, alleles: Vec<&str>) -> Variant {
    Variant {
        id,
        chrom: "chr1".to_string(),
        pos,
        alleles: alleles.into_iter().map(String::from).collect(),
    }
}

pub fn obs(id: i64, sample: i64, variant: i64, gls: Vec<f64>, q: f64) -> Observation {
    Observation {
        id,
        sample,
        variant,
        gls,
        quality: q,
        batch: "b1".to_string(),
        version: 1,
    }
}

pub fn rel(id: i64, child: i64, parent: i64, kind: ParentKind) -> Relationship {
    Relationship {
        id,
        child,
        parent,
        kind,
        status: RelStatus::Confirmed,
    }
}

pub fn find_block<'a>(view: &'a StateView, sample: i64, start: i64) -> Option<&'a PhaseBlock> {
    view.blocks.iter().find(|b| b.sample == sample && b.start_pos == start)
}

pub fn trio_bundle() -> ImportBundle {
    ImportBundle {
        samples: vec![sample(1, "S-F"), sample(2, "S-M"), sample(3, "S-C")],
        relationships: vec![
            rel(11, 3, 1, ParentKind::Father),
            rel(12, 3, 2, ParentKind::Mother),
        ],
        variants: vec![var(101, 100, vec!["A", "G"]), var(102, 200, vec!["C", "T"])],
        observations: vec![
            obs(201, 1, 101, biallelic_gls(0, 99.0), 99.0),
            obs(202, 1, 102, biallelic_gls(0, 99.0), 99.0),
            obs(203, 2, 101, biallelic_gls(1, 80.0), 80.0),
            obs(204, 2, 102, biallelic_gls(1, 80.0), 80.0),
            obs(205, 3, 101, biallelic_gls(1, 95.0), 95.0),
            obs(206, 3, 102, biallelic_gls(1, 88.0), 88.0),
        ],
        read_links: vec![],
    }
}
