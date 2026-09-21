use crate::domain::{
    GenotypeObservation, InputSnapshot, InputVersion, LinkPhase, ReadLink, Relationship,
    RelationshipStatus, Sample, Variant,
};

pub fn version(version_id: &str) -> InputSnapshot {
    let label = match version_id {
        "v2" => "关系修订批次",
        "v3" => "旧版复核快照",
        _ => "初始离线导入",
    };
    let snapshot = InputSnapshot {
        version: InputVersion {
            id: version_id.to_string(),
            label: label.to_string(),
            checksum: String::new(),
        },
        samples: vec![
            sample("f1", "ANON-1001", "male"),
            sample("m1", "ANON-1002", "female"),
            sample("c1", "ANON-1003", "female"),
            sample("p2", "ANON-1004", "male"),
            sample("m2", "ANON-1005", "female"),
            sample("c2", "ANON-1006", "male"),
        ],
        relationships: relationships(version_id),
        variants: vec![
            variant("v1", 1, 101, "A", "G"),
            variant("v2", 1, 202, "C", "T"),
            variant("v3", 1, 303, "G", "A"),
            variant("v4", 1, 404, "T", "C"),
            variant("v5", 2, 505, "A", "G"),
            variant("v6", 2, 606, "A", "G"),
        ],
        genotypes: genotypes(version_id),
        read_links: read_links(version_id),
    };
    let checksum = checksum(&snapshot);
    InputSnapshot {
        version: InputVersion {
            checksum,
            ..snapshot.version
        },
        ..snapshot
    }
}

pub fn all_versions() -> Vec<InputSnapshot> {
    vec![version("v1"), version("v2"), version("v3")]
}

fn sample(id: &str, anonymous_id: &str, sex: &str) -> Sample {
    Sample {
        id: id.to_string(),
        anonymous_id: anonymous_id.to_string(),
        sex: sex.to_string(),
    }
}

fn variant(id: &str, chromosome: i64, position: i64, reference: &str, alternate: &str) -> Variant {
    Variant {
        id: id.to_string(),
        chromosome: format!("chr{}", chromosome),
        position,
        reference: reference.to_string(),
        alternate: alternate.to_string(),
    }
}

fn relationships(version_id: &str) -> Vec<Relationship> {
    let rel2_father = if version_id == "v2" { "f1" } else { "p2" };
    let rel2_status = if version_id == "v2" {
        RelationshipStatus::Confirmed
    } else {
        RelationshipStatus::Pending
    };
    vec![
        Relationship {
            id: "rel1".to_string(),
            child_id: "c1".to_string(),
            father_id: Some("f1".to_string()),
            mother_id: Some("m1".to_string()),
            status: RelationshipStatus::Confirmed,
            weight: 10,
            note: "核心三联体".to_string(),
        },
        Relationship {
            id: "rel2".to_string(),
            child_id: "c2".to_string(),
            father_id: Some(rel2_father.to_string()),
            mother_id: Some("m2".to_string()),
            status: rel2_status,
            weight: 8,
            note: "父亲身份曾待确认".to_string(),
        },
    ]
}

fn genotypes(version_id: &str) -> Vec<GenotypeObservation> {
    let mut items = vec![
        genotype("g-f1-v1-a", "f1", "v1", "b1", Some(&["A", "G"]), 99.0),
        genotype("g-f1-v1-dup", "f1", "v1", "b2", Some(&["A", "G"]), 70.0),
        genotype("g-m1-v1", "m1", "v1", "b1", Some(&["A", "G"]), 98.0),
        genotype("g-c1-v1", "c1", "v1", "b1", Some(&["A", "G"]), 96.0),
        genotype("g-p2-v1", "p2", "v1", "b3", Some(&["A", "G"]), 91.0),
        genotype("g-m2-v1", "m2", "v1", "b3", Some(&["G", "G"]), 90.0),
        genotype("g-c2-v1", "c2", "v1", "b3", None, 5.0),
        genotype("g-f1-v2", "f1", "v2", "b1", Some(&["C", "T"]), 97.0),
        genotype("g-m1-v2", "m1", "v2", "b1", Some(&["C", "C"]), 96.0),
        genotype("g-c1-v2", "c1", "v2", "b1", Some(&["C", "T"]), 95.0),
        genotype("g-p2-v2", "p2", "v2", "b3", Some(&["T", "T"]), 89.0),
        genotype("g-m2-v2", "m2", "v2", "b3", Some(&["C", "C"]), 88.0),
        genotype("g-c2-v2", "c2", "v2", "b3", Some(&["C", "T"]), 12.0),
        genotype("g-f1-v3", "f1", "v3", "b1", Some(&["G", "A"]), 97.0),
        genotype("g-m1-v3", "m1", "v3", "b1", Some(&["G", "G"]), 96.0),
        genotype("g-c1-v3", "c1", "v3", "b1", Some(&["G", "G"]), 95.0),
        genotype("g-m2-v3", "m2", "v3", "b3", Some(&["A", "A"]), 88.0),
        genotype("g-f1-v4", "f1", "v4", "b1", Some(&["T", "C"]), 97.0),
        genotype("g-m1-v4", "m1", "v4", "b1", Some(&["T", "T"]), 96.0),
        genotype("g-c1-v4", "c1", "v4", "b1", Some(&["T", "C"]), 94.0),
        genotype("g-f1-v5", "f1", "v5", "b1", Some(&["A", "G", "T"]), 93.0),
        genotype("g-m1-v5", "m1", "v5", "b1", Some(&["A", "A"]), 96.0),
        genotype("g-c1-v5", "c1", "v5", "b1", Some(&["A", "A"]), 95.0),
        genotype("g-f1-v6", "f1", "v6", "b1", Some(&["A", "A"]), 97.0),
        genotype("g-m1-v6", "m1", "v6", "b1", Some(&["A", "A"]), 96.0),
        genotype("g-c1-v6", "c1", "v6", "b1", Some(&["G", "G"]), 88.0),
    ];

    if version_id == "v2" {
        items.push(genotype("g-c2-v3-revised", "c2", "v3", "b4", Some(&["A", "G"]), 82.0));
    }
    if version_id == "v3" {
        items.push(genotype("g-c2-v3-legacy", "c2", "v3", "b-old", Some(&["G", "G"]), 60.0));
    }
    items
}

fn genotype(
    id: &str,
    sample_id: &str,
    variant_id: &str,
    batch: &str,
    alleles: Option<&[&str]>,
    quality: f64,
) -> GenotypeObservation {
    GenotypeObservation {
        id: id.to_string(),
        sample_id: sample_id.to_string(),
        variant_id: variant_id.to_string(),
        batch: batch.to_string(),
        alleles: alleles.map(|values| values.iter().map(|value| value.to_string()).collect()),
        likelihoods: vec![quality / 100.0, 1.0 - quality / 100.0, 0.01],
        quality,
    }
}

fn read_links(version_id: &str) -> Vec<ReadLink> {
    let mut links = vec![
        read_link("rl-f1-v1-v2-cis", "f1", "v1", "v2", LinkPhase::Cis, 5, "b1"),
        read_link("rl-f1-v1-v2-trans", "f1", "v1", "v2", LinkPhase::Trans, 5, "b1"),
        read_link("rl-f1-v2-v3", "f1", "v2", "v3", LinkPhase::Cis, 7, "b1"),
        read_link("rl-f1-v3-v4-bridge", "f1", "v3", "v4", LinkPhase::Trans, 6, "b1"),
    ];
    if version_id == "v3" {
        links.push(read_link(
            "rl-f1-v2-v4-legacy",
            "f1",
            "v2",
            "v4",
            LinkPhase::Cis,
            4,
            "b-old",
        ));
    }
    links
}

fn read_link(
    id: &str,
    sample_id: &str,
    variant1_id: &str,
    variant2_id: &str,
    phase: LinkPhase,
    weight: i64,
    batch: &str,
) -> ReadLink {
    ReadLink {
        id: id.to_string(),
        sample_id: sample_id.to_string(),
        variant1_id: variant1_id.to_string(),
        variant2_id: variant2_id.to_string(),
        phase,
        weight,
        batch: batch.to_string(),
    }
}

fn checksum(snapshot: &InputSnapshot) -> String {
    let encoded = serde_json::to_string(snapshot).unwrap_or_default();
    let mut hash = 0xcbf29ce484222325u64;
    for byte in encoded.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("sha-like-{:016x}", hash)
}
