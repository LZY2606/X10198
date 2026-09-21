use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct State {
    pub current_version: Option<String>,
    pub pinned_version: Option<String>,
    pub samples: HashMap<String, Sample>,
    pub relationships: HashMap<String, Relationship>,
    pub variants: HashMap<String, Variant>,
    pub genotypes: HashMap<String, Genotype>,
    pub read_links: HashMap<String, ReadLink>,
    pub locks: HashMap<String, String>,
    pub accepted: HashMap<String, String>,
    pub withdrawn_links: BTreeSet<String>,
    pub de_novo_resolutions: BTreeSet<String>,
    pub ignored_observations: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Sample {
    pub id: String,
    pub batch: String,
    pub role_note: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Relationship {
    pub id: String,
    pub child_id: String,
    pub father_id: Option<String>,
    pub mother_id: Option<String>,
    pub batch: String,
    pub uncertain: bool,
    pub weight: i64,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Variant {
    pub id: String,
    pub chromosome: String,
    pub position: i64,
    pub reference: String,
    pub alternate: String,
    pub batch: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Genotype {
    pub id: String,
    pub sample_id: String,
    pub variant_id: String,
    pub alleles: Vec<String>,
    pub missing: bool,
    pub quality: Option<f64>,
    pub genotype_likelihoods: Vec<f64>,
    pub batch: String,
    pub import_version: String,
    pub superseded: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ReadLink {
    pub id: String,
    pub genotype_a: String,
    pub genotype_b: String,
    pub orientation: String,
    pub weight: i64,
    pub quality: Option<f64>,
    pub batch: String,
    pub import_version: String,
    pub superseded: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisView {
    pub current_version: Option<String>,
    pub pinned_version: Option<String>,
    pub samples: Vec<Sample>,
    pub relationships: Vec<Relationship>,
    pub variants: Vec<Variant>,
    pub genotypes: Vec<Genotype>,
    pub read_links: Vec<ReadLink>,
    pub blocks: Vec<BlockView>,
    pub conflicts: Vec<ConflictView>,
    pub ignored: Vec<IgnoredView>,
    pub candidate_budget: usize,
    pub budget_reached: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlockView {
    pub id: String,
    pub chromosome: String,
    pub start: i64,
    pub end: i64,
    pub nodes: Vec<String>,
    pub bridges: Vec<BridgeView>,
    pub candidates: Vec<CandidateView>,
    pub budget_reached: bool,
    pub accepted_candidate: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BridgeView {
    pub kind: String,
    pub evidence_id: String,
    pub from_node: String,
    pub to_node: String,
    pub weight: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CandidateView {
    pub id: String,
    pub fingerprint: String,
    pub rank: usize,
    pub score: i64,
    pub phases: BTreeMap<String, i32>,
    pub origins: BTreeMap<String, OriginView>,
    pub supporting: Vec<SupportView>,
    pub ignored_data: Vec<String>,
    pub equivalent: Vec<String>,
    pub budget_truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct OriginView {
    pub father_allele: String,
    pub mother_allele: String,
    pub swapped_equivalent: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SupportView {
    pub relation_id: String,
    pub kind: String,
    pub weight: i64,
    pub observations: Vec<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConflictView {
    pub id: String,
    pub kind: String,
    pub chromosome: String,
    pub position: i64,
    pub variant_id: String,
    pub relationship_id: Option<String>,
    pub observations: Vec<String>,
    pub detail: String,
    pub minimum_sets: Vec<Vec<String>>,
    pub de_novo_candidates: Vec<String>,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IgnoredView {
    pub observation_id: String,
    pub reason: String,
    pub weight: i64,
}

#[derive(Clone)]
struct DSU {
    parent: Vec<usize>,
    size: Vec<usize>,
}

impl DSU {
    fn new(n: usize) -> Self {
        Self { parent: (0..n).collect(), size: vec![1; n] }
    }
    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }
    fn union(&mut self, a: usize, b: usize) -> bool {
        let (a, b) = (self.find(a), self.find(b));
        if a == b { return false; }
        if self.size[a] < self.size[b] { self.parent[a] = b; self.size[b] += self.size[a]; }
        else { self.parent[b] = a; self.size[a] += self.size[b]; }
        true
    }
}

#[derive(Clone)]
struct ParityDSU {
    dsu: DSU,
    xor: Vec<i32>,
}

impl ParityDSU {
    fn new(n: usize) -> Self {
        Self { dsu: DSU::new(n), xor: vec![0; n] }
    }
    fn parity(&mut self, x: usize) -> (usize, i32) {
        let root = self.dsu.find(x);
        let mut cur = x;
        let mut value = 0;
        while cur != root {
            value ^= self.xor[cur];
            cur = self.dsu.parent[cur];
        }
        (root, value)
    }
    fn add(&mut self, a: usize, b: usize, edge_xor: i32) -> bool {
        let (ra, xa) = self.parity(a);
        let (rb, xb) = self.parity(b);
        if ra == rb { return (xa ^ xb) == edge_xor; }
        self.dsu.union(ra, rb);
        let new_root = self.dsu.find(ra);
        let old_root = if new_root == ra { rb } else { ra };
        self.xor[old_root] = xa ^ xb ^ edge_xor;
        true
    }
}

impl State {
    pub fn new() -> Self { Self::default() }

    pub fn apply(&mut self, kind: &str, payload: serde_json::Value) {
        match kind {
            "import" => self.apply_import(payload),
            "pin_version" => {
                self.pinned_version = payload.get("version").and_then(value_string);
            }
            "lock_phase" | "unlock_phase" => {
                if let Some(node) = payload.get("genotype_id").and_then(value_string) {
                    if kind == "lock_phase" {
                        if let Some(bit) = payload.get("allele_index").and_then(|v| v.as_i64()) {
                            let phase = if bit == 0 { "A" } else { "B" };
                            self.locks.insert(node, phase.to_string());
                        } else if let Some(phase) = payload.get("phase").and_then(value_string) {
                            self.locks.insert(node, phase);
                        }
                    } else {
                        self.locks.remove(&node);
                    }
                }
            }
            "accept_candidate" => {
                if let Some(block) = payload.get("block_id").and_then(value_string) {
                    if let Some(fingerprint) = payload.get("fingerprint").and_then(value_string) {
                        self.accepted.insert(block, fingerprint);
                    }
                }
            }
            "withdraw_read_link" | "restore_read_link" => {
                if let Some(id) = payload.get("read_link_id").and_then(value_string) {
                    if kind == "withdraw_read_link" { self.withdrawn_links.insert(id); }
                    else { self.withdrawn_links.remove(&id); }
                }
            }
            "ignore_observation" | "restore_observation" => {
                if let Some(id) = payload.get("observation_id").and_then(value_string) {
                    if kind == "ignore_observation" { self.ignored_observations.insert(id); }
                    else { self.ignored_observations.remove(&id); }
                }
            }
            "resolve_de_novo" | "reopen_de_novo" => {
                if let Some(id) = payload.get("conflict_id").and_then(value_string) {
                    if kind == "resolve_de_novo" { self.de_novo_resolutions.insert(id); }
                    else { self.de_novo_resolutions.remove(&id); }
                }
            }
            "revise_relationship" => self.apply_relation_revision(payload),
            _ => {}
        }
    }

    fn apply_import(&mut self, payload: serde_json::Value) {
        let version = payload.get("version").and_then(value_string).unwrap_or_default();
        self.current_version = Some(version.clone());
        if self.pinned_version.is_none() { self.pinned_version = Some(version.clone()); }
        self.samples.clear();
        self.relationships.clear();
        self.variants.clear();
        self.genotypes.clear();
        self.read_links.clear();
        self.locks.clear();
        self.accepted.clear();
        self.withdrawn_links.clear();
        self.de_novo_resolutions.clear();
        self.ignored_observations.clear();
        if let Some(values) = payload.get("samples").and_then(|v| v.as_array()) {
            for value in values {
                let mut sample: Sample = serde_json::from_value(value.clone()).unwrap_or_default();
                if sample.batch.is_empty() { sample.batch = current_batch(&payload); }
                self.samples.insert(sample.id.clone(), sample);
            }
        }
        if let Some(values) = payload.get("relationships").and_then(|v| v.as_array()) {
            for value in values {
                let mut relation: Relationship = serde_json::from_value(value.clone()).unwrap_or_default();
                if relation.batch.is_empty() { relation.batch = current_batch(&payload); }
                if relation.weight == 0 { relation.weight = 1; }
                self.relationships.insert(relation.id.clone(), relation);
            }
        }
        if let Some(values) = payload.get("variants").and_then(|v| v.as_array()) {
            for value in values {
                let mut variant: Variant = serde_json::from_value(value.clone()).unwrap_or_default();
                if variant.batch.is_empty() { variant.batch = current_batch(&payload); }
                self.variants.insert(variant.id.clone(), variant);
            }
        }
        if let Some(values) = payload.get("genotypes").and_then(|v| v.as_array()) {
            for value in values {
                let mut genotype: Genotype = serde_json::from_value(value.clone()).unwrap_or_default();
                if genotype.batch.is_empty() { genotype.batch = current_batch(&payload); }
                genotype.import_version = version.clone();
                self.genotypes.insert(genotype.id.clone(), genotype);
            }
        }
        if let Some(values) = payload.get("read_links").and_then(|v| v.as_array()) {
            for value in values {
                let mut link: ReadLink = serde_json::from_value(value.clone()).unwrap_or_default();
                if link.batch.is_empty() { link.batch = current_batch(&payload); }
                if link.weight == 0 { link.weight = 1; }
                link.import_version = version.clone();
                self.read_links.insert(link.id.clone(), link);
            }
        }
    }

    fn apply_relation_revision(&mut self, payload: serde_json::Value) {
        let Some(id) = payload.get("relationship_id").and_then(value_string) else { return; };
        let Some(relation) = self.relationships.get_mut(&id) else { return; };
        if let Some(value) = payload.get("father_id") {
            relation.father_id = if value.is_null() { None } else { value_string(value) };
        }
        if let Some(value) = payload.get("mother_id") {
            relation.mother_id = if value.is_null() { None } else { value_string(value) };
        }
        if let Some(value) = payload.get("uncertain").and_then(|v| v.as_bool()) {
            relation.uncertain = value;
            relation.confirmed = !value;
        }
        if let Some(value) = payload.get("confirmed").and_then(|v| v.as_bool()) {
            relation.confirmed = value;
            if value { relation.uncertain = false; }
        }
        if let Some(value) = payload.get("weight").and_then(|v| v.as_i64()) {
            relation.weight = value;
        }
    }

    pub fn analyze(&self, candidate_budget: usize) -> AnalysisView {
        let active = self.active_genotypes();
        let mut ignored = self.global_ignored(&active);
        let conflicts = self.conflicts(&active, &mut ignored);
        let valid: HashMap<_, _> = active.iter()
            .filter(|g| !ignored.iter().any(|item| item.observation_id == g.id) && self.valid_biallelic(g))
            .map(|g| (g.id.clone(), g.clone()))
            .collect();
        let (blocks, mut budget_reached) = self.build_blocks(&valid);
        let mut blocks: Vec<BlockView> = blocks.into_iter().map(|mut block| {
            let candidates = self.candidates(&block, &valid, candidate_budget);
            block.budget_reached = candidates.iter().any(|c| c.budget_truncated);
            if block.budget_reached { budget_reached = true; }
            block.candidates = candidates;
            block.accepted_candidate = self.accepted.get(&block.id).cloned();
            block
        }).collect();
        blocks.sort_by(|a, b| (a.chromosome.clone(), a.start, a.id.clone()).cmp(&(b.chromosome.clone(), b.start, b.id.clone())));
        AnalysisView {
            current_version: self.current_version.clone(),
            pinned_version: self.pinned_version.clone(),
            samples: values_sorted(&self.samples),
            relationships: values_sorted(&self.relationships),
            variants: values_sorted(&self.variants),
            genotypes: values_sorted(&self.genotypes),
            read_links: values_sorted(&self.read_links),
            blocks,
            conflicts,
            ignored,
            candidate_budget,
            budget_reached,
        }
    }

    fn active_genotypes(&self) -> Vec<Genotype> {
        let version = self.pinned_version.as_ref().or(self.current_version.as_ref());
        self.genotypes.values().filter(|g| !g.superseded && version.is_some_and(|v| v == &g.import_version)).cloned().collect()
    }

    fn global_ignored(&self, active: &[Genotype]) -> Vec<IgnoredView> {
        let mut ignored = Vec::new();
        for genotype in active {
            let mut reason = None;
            if genotype.missing { reason = Some("missing genotype"); }
            else if !self.samples.contains_key(&genotype.sample_id) || !self.variants.contains_key(&genotype.variant_id) {
                reason = Some("unknown sample or variant");
            } else if genotype.alleles.len() == 1 || genotype.alleles.len() > 2 || genotype.alleles.iter().any(allele_bad) {
                reason = Some("invalid or triallelic call");
            } else if genotype.quality.is_some_and(|q| q < 20.0) {
                reason = Some("low-quality heterozygous genotype");
            } else if let Some(weight) = genotype_weight(genotype) {
                if weight <= 0 { reason = Some("zero likelihood support"); }
            }
            if let Some(reason) = reason {
                ignored.push(IgnoredView { observation_id: genotype.id.clone(), reason: reason.to_string(), weight: genotype_weight(genotype).unwrap_or(0) });
            }
        }
        for link in self.read_links.values() {
            let active_link = self.pinned_version.as_ref().or(self.current_version.as_ref()).is_some_and(|v| v == &link.import_version) && !link.superseded;
            let mut reason = None;
            if !active_link { reason = Some("link is not in pinned version"); }
            else if self.withdrawn_links.contains(&link.id) { reason = Some("read link withdrawn"); }
            else {
                let endpoints: Vec<_> = [&link.genotype_a, &link.genotype_b].iter().filter_map(|id| self.genotypes.get(*id)).collect();
                if endpoints.len() != 2 || endpoints[0].sample_id != endpoints[1].sample_id {
                    reason = Some("read link endpoints are invalid");
                } else if link.quality.is_some_and(|q| q < 20.0) || link.weight <= 0 {
                    reason = Some("low-quality read link");
                }
            }
            if let Some(reason) = reason {
                ignored.push(IgnoredView { observation_id: link.id.clone(), reason: reason.to_string(), weight: link.weight });
            }
        }
        for relation in self.relationships.values() {
            if relation.uncertain {
                ignored.push(IgnoredView { observation_id: relation.id.clone(), reason: "sample relationship uncertain".to_string(), weight: relation.weight });
            }
        }
        for id in &self.ignored_observations {
            if !ignored.iter().any(|item| &item.observation_id == id) {
                ignored.push(IgnoredView { observation_id: id.clone(), reason: "manually isolated".to_string(), weight: 0 });
            }
        }
        ignored.sort_by(|a, b| a.observation_id.cmp(&b.observation_id));
        ignored.dedup_by(|a, b| a.observation_id == b.observation_id);
        ignored
    }

    fn conflicts(&self, active: &[Genotype], ignored: &mut Vec<IgnoredView>) -> Vec<ConflictView> {
        let mut conflicts = Vec::new();
        let mut groups: HashMap<(String, String), Vec<&Genotype>> = HashMap::new();
        for genotype in active {
            groups.entry((genotype.sample_id.clone(), genotype.variant_id.clone())).or_default().push(genotype);
        }
        for ((sample_id, variant_id), observations) in groups {
            if observations.len() < 2 { continue; }
            let calls: BTreeSet<Vec<String>> = observations.iter()
                .filter(|g| !g.missing && g.alleles.len() == 2)
                .map(|g| normalized_pair(&g.alleles))
                .collect();
            let kind = if calls.len() > 1 { "duplicate_conflicting_observation" } else { "duplicate_observation" };
            if calls.len() > 1 {
                let ids: Vec<String> = observations.iter().map(|g| g.id.clone()).collect();
                let variant = self.variants.get(&variant_id);
                conflicts.push(ConflictView {
                    id: format!("conflict:{}:{}", kind, ids.join("+")),
                    kind: kind.to_string(),
                    chromosome: variant.map(|v| v.chromosome.clone()).unwrap_or_default(),
                    position: variant.map(|v| v.position).unwrap_or(0),
                    variant_id,
                    relationship_id: None,
                    observations: ids,
                    detail: format!("sample {sample_id} has repeated genotype calls with distinct alleles"),
                    minimum_sets: minimal_repairs(&observations.iter().map(|g| g.id.clone()).collect::<Vec<_>>(), |removed| {
                        calls_after(&observations, removed).len() <= 1
                    }),
                    de_novo_candidates: vec![],
                    resolution: None,
                });
            } else {
                for id in observations.iter().skip(1).map(|g| g.id.clone()) {
                    ignored.push(IgnoredView { observation_id: id, reason: "duplicate repeated observation".to_string(), weight: genotype_weight(observations[0]).unwrap_or(0) });
                }
            }
        }
        for relation in self.relationships.values().filter(|r| r.uncertain) {
            conflicts.push(ConflictView {
                id: format!("conflict:uncertain_relation:{}", relation.id),
                kind: "relationship_uncertain".to_string(),
                chromosome: "*".to_string(),
                position: 0,
                variant_id: "*".to_string(),
                relationship_id: Some(relation.id.clone()),
                observations: vec![relation.id.clone()],
                detail: "parent identity is not confirmed; transmission evidence is withheld".to_string(),
                minimum_sets: vec![],
                de_novo_candidates: vec![],
                resolution: None,
            });
        }
        for relation in self.relationships.values().filter(|r| !r.uncertain) {
            for variant in self.variants.values() {
                let children: Vec<&Genotype> = active.iter().filter(|g| g.sample_id == relation.child_id && g.variant_id == variant.id && !g.missing).collect();
                let fathers: Vec<&Genotype> = relation.father_id.as_ref().map(|id| active.iter().filter(|g| &g.sample_id == id && g.variant_id == variant.id && !g.missing).collect()).unwrap_or_default();
                let mothers: Vec<&Genotype> = relation.mother_id.as_ref().map(|id| active.iter().filter(|g| &g.sample_id == id && g.variant_id == variant.id && !g.missing).collect()).unwrap_or_default();
                if children.is_empty() {
                    conflicts.push(self.missing_conflict(variant, Some(relation), "child genotype is missing", &format!("conflict:missing:{}:{}:child", relation.id, variant.id)));
                    continue;
                }
                if relation.father_id.is_some() && fathers.is_empty() {
                    conflicts.push(self.missing_conflict(variant, Some(relation), "father genotype is missing", &format!("conflict:missing:{}:{}:father", relation.id, variant.id)));
                }
                if relation.mother_id.is_some() && mothers.is_empty() {
                    conflicts.push(self.missing_conflict(variant, Some(relation), "mother genotype is missing", &format!("conflict:missing:{}:{}:mother", relation.id, variant.id)));
                }
                for child in &children {
                    for father in optional_matrix(&fathers, relation.father_id.is_some()) {
                        for mother in optional_matrix(&mothers, relation.mother_id.is_some()) {
                            if let Some(conflict) = self.mendelian_conflict(variant, relation, child, father, mother) {
                                conflicts.push(conflict);
                            }
                        }
                    }
                }
            }
        }
        conflicts.extend(self.read_parity_conflicts(active));
        conflicts.sort_by(|a, b| (a.chromosome.clone(), a.position, a.id.clone()).cmp(&(b.chromosome.clone(), b.position, b.id.clone())));
        conflicts.dedup_by(|a, b| a.id == b.id);
        conflicts
    }

    fn missing_conflict(&self, variant: &Variant, relation: Option<&Relationship>, detail: &str, id: &str) -> ConflictView {
        ConflictView {
            id: id.to_string(),
            kind: "missing_genotype".to_string(),
            chromosome: variant.chromosome.clone(),
            position: variant.position,
            variant_id: variant.id.clone(),
            relationship_id: relation.map(|r| r.id.clone()),
            observations: relation.map(|r| vec![r.id.clone()]).unwrap_or_default(),
            detail: detail.to_string(),
            minimum_sets: vec![],
            de_novo_candidates: vec![],
            resolution: None,
        }
    }

    fn mendelian_conflict(&self, variant: &Variant, relation: &Relationship, child: &Genotype, father: Option<&&Genotype>, mother: Option<&&Genotype>) -> Option<ConflictView> {
        let child_ok = valid_pair(child);
        let father_ok = father.is_none() || father.is_some_and(|g| valid_pair(g));
        let mother_ok = mother.is_none() || mother.is_some_and(|g| valid_pair(g));
        let child_alleles = if child_ok { Some(normalized_pair(&child.alleles)) } else { None };
        let father_alleles = if father_ok { father.map(|g| normalized_pair(&g.alleles)) } else { None };
        let mother_alleles = if mother_ok { mother.map(|g| normalized_pair(&g.alleles)) } else { None };
        let compatible = child_alleles.as_ref().is_some_and(|child_pair| {
            father_alleles.as_ref().is_none_or(|f| child_pair.iter().any(|a| f.contains(a)))
                && mother_alleles.as_ref().is_none_or(|m| child_pair.iter().any(|a| m.contains(a)))
                && pair_inheritance(child_pair, father_alleles.as_deref(), mother_alleles.as_deref())
        });
        let triallelic = child.alleles.len() > 2 || father.is_some_and(|g| g.alleles.len() > 2) || mother.is_some_and(|g| g.alleles.len() > 2);
        if compatible || (!triallelic && child_alleles.is_none()) { return None; }
        let mut observations = vec![child.id.clone()];
        if let Some(father) = father { observations.push(father.id.clone()); }
        if let Some(mother) = mother { observations.push(mother.id.clone()); }
        let all_alleles: BTreeSet<String> = father_alleles.iter().flatten().chain(mother_alleles.iter().flatten()).cloned().collect();
        let de_novo: Vec<String> = child_alleles.iter().flatten().filter(|a| !all_alleles.contains(*a))
            .map(|a| format!("{}:{}", variant.id, a)).collect();
        let minimum_sets = minimal_repairs(&observations, |removed| {
            if removed.contains(&child.id) { return false; }
            let f_id = father.map(|g| g.id.clone());
            let m_id = mother.map(|g| g.id.clone());
            if f_id.as_ref().is_some_and(|id| !removed.contains(id)) && m_id.as_ref().is_some_and(|id| !removed.contains(id)) {
                let fp = father_alleles.as_ref().unwrap();
                let mp = mother_alleles.as_ref().unwrap();
                child_alleles.as_ref().is_some_and(|cp| pair_inheritance(cp, Some(fp), Some(mp)))
            } else if f_id.as_ref().is_some_and(|id| !removed.contains(id)) {
                child_alleles.as_ref().is_some_and(|cp| cp.iter().any(|a| father_alleles.as_ref().unwrap().contains(a)))
            } else if m_id.as_ref().is_some_and(|id| !removed.contains(id)) {
                child_alleles.as_ref().is_some_and(|cp| cp.iter().any(|a| mother_alleles.as_ref().unwrap().contains(a)))
            } else {
                false
            }
        });
        let id = format!("conflict:mendelian:{}:{}:{}", relation.id, variant.id, observations.join("+"));
        let resolution = if self.de_novo_resolutions.contains(&id) { Some("accepted_de_novo".to_string()) } else { None };
        Some(ConflictView {
            id,
            kind: if triallelic { "triallelic_mendelian_conflict".to_string() } else { "mendelian_inconsistency".to_string() },
            chromosome: variant.chromosome.clone(),
            position: variant.position,
            variant_id: variant.id.clone(),
            relationship_id: Some(relation.id.clone()),
            observations,
            detail: if triallelic { "a three-allele call cannot satisfy ordinary Mendelian transmission".to_string() } else { "no parental allele assignment explains the child genotype".to_string() },
            minimum_sets,
            de_novo_candidates: de_novo,
            resolution,
        })
    }

    fn read_parity_conflicts(&self, active: &[Genotype]) -> Vec<ConflictView> {
        let valid_ids: HashSet<String> = active.iter().filter(|g| self.valid_biallelic(g) && g.alleles.len() == 2 && g.alleles[0] != g.alleles[1]).map(|g| g.id.clone()).collect();
        let mut by_chromosome: HashMap<String, Vec<&ReadLink>> = HashMap::new();
        for link in self.read_links.values() {
            if self.withdrawn_links.contains(&link.id) || link.weight <= 0 || link.quality.is_some_and(|q| q < 20.0) { continue; }
            if !valid_ids.contains(&link.genotype_a) || !valid_ids.contains(&link.genotype_b) { continue; }
            let ga = self.genotypes.get(&link.genotype_a);
            let gb = self.genotypes.get(&link.genotype_b);
            if let (Some(ga), Some(gb)) = (ga, gb) {
                if ga.sample_id == gb.sample_id {
                    let variant = self.variants.get(&ga.variant_id);
                    by_chromosome.entry(variant.map(|v| v.chromosome.clone()).unwrap_or_default()).or_default().push(link);
                }
            }
        }
        let mut result = Vec::new();
        for links in by_chromosome.into_values() {
            let mut ids: Vec<String> = links.iter().flat_map(|l| [l.genotype_a.clone(), l.genotype_b.clone()]).collect();
            ids.sort(); ids.dedup();
            let index: HashMap<String, usize> = ids.iter().enumerate().map(|(i, id)| (id.clone(), i)).collect();
            let mut dsu = ParityDSU::new(ids.len());
            let mut bad = Vec::new();
            for link in &links {
                let expected = if link.orientation.eq_ignore_ascii_case("opposite") { 1 } else { 0 };
                let ok = dsu.add(index[&link.genotype_a], index[&link.genotype_b], expected);
                if !ok { bad.push(link); }
            }
            for link in bad {
                let variant = self.genotypes.get(&link.genotype_a).and_then(|g| self.variants.get(&g.variant_id));
                let link_ids: Vec<String> = links.iter().map(|l| l.id.clone()).collect();
                let repairs = minimal_repairs(&link_ids, |removed| {
                    let mut test = ParityDSU::new(ids.len());
                    links.iter().filter(|l| !removed.contains(&l.id)).all(|l| {
                        let expected = if l.orientation.eq_ignore_ascii_case("opposite") { 1 } else { 0 };
                        test.add(index[&l.genotype_a], index[&l.genotype_b], expected)
                    })
                });
                result.push(ConflictView {
                    id: format!("conflict:read_parity:{}", link.id),
                    kind: "read_link_parity_conflict".to_string(),
                    chromosome: variant.map(|v| v.chromosome.clone()).unwrap_or_default(),
                    position: variant.map(|v| v.position).unwrap_or(0),
                    variant_id: variant.map(|v| v.id.clone()).unwrap_or_default(),
                    relationship_id: None,
                    observations: vec![link.id.clone(), link.genotype_a.clone(), link.genotype_b.clone()],
                    detail: "read links impose contradictory same/opposite phase parity".to_string(),
                    minimum_sets: repairs,
                    de_novo_candidates: vec![],
                    resolution: None,
                });
            }
        }
        result
    }

    fn build_blocks(&self, valid: &HashMap<String, Genotype>) -> (Vec<BlockView>, bool) {
        let mut node_ids: Vec<String> = valid.keys().cloned().collect();
        node_ids.sort_by(|a, b| {
            let ga = &valid[a]; let gb = &valid[b];
            let va = self.variants.get(&ga.variant_id);
            let vb = self.variants.get(&gb.variant_id);
            (va.map(|v| v.chromosome.as_str()).unwrap_or(""), va.map(|v| v.position).unwrap_or(0), ga.sample_id.as_str(), a.as_str())
                .cmp(&(vb.map(|v| v.chromosome.as_str()).unwrap_or(""), vb.map(|v| v.position).unwrap_or(0), gb.sample_id.as_str(), b.as_str()))
        });
        let index: HashMap<String, usize> = node_ids.iter().enumerate().map(|(i, id)| (id.clone(), i)).collect();
        let mut dsu = DSU::new(node_ids.len());
        let mut bridges: Vec<(usize, usize, BridgeView)> = Vec::new();
        for link in self.read_links.values() {
            if self.withdrawn_links.contains(&link.id) { continue; }
            if !valid.contains_key(&link.genotype_a) || !valid.contains_key(&link.genotype_b) { continue; }
            let ga = &valid[&link.genotype_a];
            let gb = &valid[&link.genotype_b];
            if ga.sample_id != gb.sample_id { continue; }
            dsu.union(index[&link.genotype_a], index[&link.genotype_b]);
            bridges.push((index[&link.genotype_a], index[&link.genotype_b], BridgeView {
                kind: "read_link".to_string(),
                evidence_id: link.id.clone(),
                from_node: link.genotype_a.clone(),
                to_node: link.genotype_b.clone(),
                weight: link.weight,
            }));
        }
        for relation in self.relationships.values().filter(|r| !r.uncertain) {
            for variant_id in self.variants.keys() {
                let child: Vec<&String> = valid.values().filter(|g| g.sample_id == relation.child_id && &g.variant_id == variant_id).map(|g| &g.id).collect();
                let father: Vec<&String> = relation.father_id.as_ref().map(|pid| valid.values().filter(|g| &g.sample_id == pid && &g.variant_id == variant_id).map(|g| &g.id).collect()).unwrap_or_default();
                let mother: Vec<&String> = relation.mother_id.as_ref().map(|pid| valid.values().filter(|g| &g.sample_id == pid && &g.variant_id == variant_id).map(|g| &g.id).collect()).unwrap_or_default();
                for c in &child {
                    for p in father.iter().chain(mother.iter()) {
                        dsu.union(index[*c], index[*p]);
                        bridges.push((*index.get(*c).unwrap(), index[*p], BridgeView {
                            kind: "parent_child_transmission".to_string(),
                            evidence_id: relation.id.clone(),
                            from_node: (*c).clone(),
                            to_node: (*p).clone(),
                            weight: relation.weight,
                        }));
                    }
                }
            }
        }
        let mut components: HashMap<usize, Vec<String>> = HashMap::new();
        for id in &node_ids {
            components.entry(dsu.find(index[id])).or_default().push(id.clone());
        }
        let mut blocks = Vec::new();
        for (root, nodes) in components {
            let mut chromosomes: BTreeSet<String> = nodes.iter().filter_map(|id| self.variants.get(&valid[id].variant_id).map(|v| v.chromosome.clone())).collect();
            let chromosome = chromosomes.pop_first().unwrap_or_else(|| "unknown".to_string());
            let positions: Vec<i64> = nodes.iter().filter_map(|id| self.variants.get(&valid[id].variant_id).map(|v| v.position)).collect();
            let block_id = format!("block:{chromosome}:{}", fingerprint_str(&nodes));
            let block_bridges: Vec<BridgeView> = bridges.iter()
                .filter(|(a, b, _)| dsu.find(*a) == root && dsu.find(*b) == root)
                .map(|(_, _, bridge)| bridge.clone()).collect();
            blocks.push(BlockView {
                id: block_id,
                chromosome,
                start: positions.iter().copied().min().unwrap_or(0),
                end: positions.iter().copied().max().unwrap_or(0),
                nodes,
                bridges: block_bridges,
                candidates: Vec::new(),
                budget_reached: false,
                accepted_candidate: None,
            });
        }
        (blocks, false)
    }

    fn candidates(&self, block: &BlockView, valid: &HashMap<String, Genotype>, budget: usize) -> Vec<CandidateView> {
        let mut het_nodes: Vec<String> = block.nodes.iter().filter(|id| {
            let genotype = &valid[*id];
            genotype.alleles.len() == 2 && genotype.alleles[0] != genotype.alleles[1]
        }).cloned().collect();
        het_nodes.sort();
        let read_constraints: Vec<ReadConstraint> = block.bridges.iter().filter_map(|bridge| {
            if bridge.kind != "read_link" { return None; }
            let link = self.read_links.get(&bridge.evidence_id)?;
            if !valid.contains_key(&link.genotype_a) || !valid.contains_key(&link.genotype_b) { return None; }
            let ga = &valid[&link.genotype_a];
            let gb = &valid[&link.genotype_b];
            if ga.sample_id != gb.sample_id || ga.alleles.len() != 2 || gb.alleles.len() != 2 || ga.alleles[0] == ga.alleles[1] || gb.alleles[0] == gb.alleles[1] { return None; }
            Some(ReadConstraint {
                id: link.id.clone(),
                a: link.genotype_a.clone(),
                b: link.genotype_b.clone(),
                required: if link.orientation.eq_ignore_ascii_case("opposite") { 1 } else { 0 },
                weight: link.weight,
            })
        }).collect();
        let mut trans_constraints = Vec::new();
        for relation in self.relationships.values().filter(|r| !r.uncertain) {
            for node in &block.nodes {
                let child = &valid[node];
                let variant = match self.variants.get(&child.variant_id) { Some(v) => v, None => continue };
                if child.sample_id != relation.child_id || child.alleles.len() != 2 { continue; }
                let father = relation.father_id.as_ref().and_then(|pid| block.nodes.iter().find_map(|id| if &valid[id].sample_id == pid && valid[id].variant_id == variant.id { Some(id) } else { None }));
                let mother = relation.mother_id.as_ref().and_then(|pid| block.nodes.iter().find_map(|id| if &valid[id].sample_id == pid && valid[id].variant_id == variant.id { Some(id) } else { None }));
                if let Some(father) = father { trans_constraints.push(self.trans_constraint(relation, variant, node, father, "father")); }
                if let Some(mother) = mother { trans_constraints.push(self.trans_constraint(relation, variant, node, mother, "mother")); }
            }
        }
        let total_bits = het_nodes.len() + trans_constraints.len();
        let budget_reached = total_bits >= 10 || 1usize << total_bits.min(20) > budget;
        let combinations = (0..(1usize << total_bits.min(10))).take(budget).collect::<Vec<_>>();
        let mut scored = Vec::new();
        for combination in combinations {
            let mut phases = BTreeMap::new();
            for (i, node) in het_nodes.iter().enumerate() {
                let genotype = &valid[node];
                phases.insert(node.clone(), if (combination >> i) & 1 == 0 { 0 } else { 1 });
                let _ = genotype;
            }
            let mut origins_index = BTreeMap::new();
            for (i, constraint) in trans_constraints.iter().enumerate() {
                origins_index.insert(constraint.id.clone(), if (combination >> (het_nodes.len() + i)) & 1 == 0 { 0 } else { 1 });
            }
            if let Some(lock) = self.lock_violation(&het_nodes, &phases) {
                score -= 1_000_000;
                ignored_data.push(format!("locked-phase:{lock}"));
            }
            let mut score = 0i64;
            let mut supporting = Vec::new();
            let mut ignored_data = Vec::new();
            for constraint in &read_constraints {
                let xa = *phases.get(&constraint.a).unwrap_or(&0);
                let xb = *phases.get(&constraint.b).unwrap_or(&0);
                let matched = (xa ^ xb) == constraint.required;
                if matched {
                    score += constraint.weight;
                    supporting.push(SupportView {
                        relation_id: constraint.id.clone(),
                        kind: "read_link".to_string(),
                        weight: constraint.weight,
                        observations: vec![constraint.a.clone(), constraint.b.clone()],
                        detail: if constraint.required == 1 { "opposite haplotype read support".to_string() } else { "same haplotype read support".to_string() },
                    });
                } else {
                    ignored_data.push(constraint.id.clone());
                }
            }
            for constraint in &trans_constraints {
                let child_phase = *phases.get(&constraint.child).unwrap_or(&0);
                let parent_phase = *origins_index.get(&constraint.id).unwrap_or(&0);
                let child_allele = valid[&constraint.child].alleles[child_phase as usize].clone();
                let parent = &valid[&constraint.parent];
                let parent_allele = parent.alleles.get(parent_phase as usize).cloned().unwrap_or_default();
                let matched = child_allele == parent_allele;
                if matched {
                    score += constraint.weight;
                    supporting.push(SupportView {
                        relation_id: constraint.id.clone(),
                        kind: "parent_child_transmission".to_string(),
                        weight: constraint.weight,
                        observations: vec![constraint.child.clone(), constraint.parent.clone()],
                        detail: format!("{} allele {} received from {}", constraint.child, child_allele, constraint.parent_role),
                    });
                } else {
                    ignored_data.push(format!("{}:{}", constraint.id, constraint.parent_role));
                }
            }
            scored.push((score, phases, origins_index, ignored_data, supporting));
        }
        let best = scored.iter().map(|item| item.0).max().unwrap_or(0);
        let mut tied: Vec<_> = scored.into_iter().filter(|item| item.0 == best).collect();
        tied.sort_by_cached_key(|item| fingerprint_phases(&item.1));
        tied.truncate(budget);
        tied.into_iter().enumerate().map(|(rank, item)| {
            let (score, phases, origins_index, ignored_data, supporting) = item;
            let fingerprint = fingerprint_phases(&phases);
            let complement: BTreeMap<String, i32> = phases.iter().map(|(k, v)| (k.clone(), v ^ 1)).collect();
            let equivalent = if self.complement_possible(block, valid) { vec![fingerprint_phases(&complement)] } else { vec![] };
            CandidateView {
                id: format!("candidate:{fingerprint}"),
                fingerprint,
                rank: rank + 1,
                score,
                phases,
                origins: self.origin_view(&origins_index, &trans_constraints, valid),
                supporting,
                ignored_data,
                equivalent,
                budget_truncated: budget_reached,
            }
        }).collect()
    }

    fn lock_violation(&self, nodes: &[String], phases: &BTreeMap<String, i32>) -> Option<String> {
        for node in nodes {
            if let Some(locked) = self.locks.get(node) {
                let expected = if locked == "A" { 0 } else { 1 };
                if phases[node] != expected { return Some(node.clone()); }
            }
        }
        None
    }

    fn trans_constraint(&self, relation: &Relationship, variant: &Variant, child: &str, parent: &str, role: &str) -> TransConstraint {
        TransConstraint {
            id: format!("{}:{}:{}:{}", relation.id, variant.id, child, role),
            child: child.to_string(),
            parent: parent.to_string(),
            parent_role: role.to_string(),
            weight: relation.weight.max(1),
        }
    }

    fn origin_view(&self, origins: &BTreeMap<String, i32>, constraints: &[TransConstraint], valid: &HashMap<String, Genotype>) -> BTreeMap<String, OriginView> {
        let mut result = BTreeMap::new();
        let mut by_child: BTreeMap<String, Vec<&TransConstraint>> = BTreeMap::new();
        for constraint in constraints { by_child.entry(constraint.child.clone()).or_default().push(constraint); }
        for (child_id, items) in by_child {
            let child = match valid.get(&child_id) { Some(g) => g, None => continue };
            let mut father_allele = "?".to_string();
            let mut mother_allele = "?".to_string();
            for item in items {
                let bit = origins.get(&item.id).copied().unwrap_or(0);
                let allele = valid.get(&item.parent).and_then(|g| g.alleles.get(bit as usize)).cloned().unwrap_or("?".to_string());
                if item.parent_role == "father" { father_allele = allele; } else { mother_allele = allele; }
            }
            let swapped_equivalent = child.alleles.len() == 2 && child.alleles[0] != child.alleles[1]
                && father_allele != mother_allele && child.alleles.contains(&father_allele) && child.alleles.contains(&mother_allele);
            result.insert(child_id, OriginView { father_allele, mother_allele, swapped_equivalent });
        }
        result
    }

    fn complement_possible(&self, block: &BlockView, valid: &HashMap<String, Genotype>) -> bool {
        block.nodes.iter().filter_map(|id| valid.get(id)).any(|g| {
            self.relationships.values().any(|r| r.uncertain && (r.child_id == g.sample_id))
        }) || block.nodes.iter().all(|id| !self.locks.contains_key(id))
    }

    fn valid_biallelic(&self, genotype: &Genotype) -> bool {
        self.samples.contains_key(&genotype.sample_id)
            && self.variants.contains_key(&genotype.variant_id)
            && !genotype.missing
            && genotype.alleles.len() == 2
            && genotype.alleles.iter().all(|a| !allele_bad(a))
            && !genotype.quality.is_some_and(|q| q < 20.0)
            && genotype_weight(genotype).is_none_or(|w| w > 0)
    }
}

#[derive(Clone)]
struct ReadConstraint {
    id: String,
    a: String,
    b: String,
    required: i32,
    weight: i64,
}

#[derive(Clone)]
struct TransConstraint {
    id: String,
    child: String,
    parent: String,
    parent_role: String,
    weight: i64,
}

fn values_sorted<T: Clone, K: Ord>(map: &HashMap<String, T>) -> Vec<T> {
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    keys.iter().map(|key| map[*key].clone()).collect()
}

fn value_string(value: &serde_json::Value) -> Option<String> {
    value.as_str().map(str::to_string)
}

fn current_batch(payload: &serde_json::Value) -> String {
    payload.get("batch").and_then(value_string).unwrap_or_default()
}

fn allele_bad(allele: &str) -> bool {
    allele.is_empty() || allele == "." || allele == "-"
}

fn valid_pair(genotype: &Genotype) -> bool {
    genotype.alleles.len() == 2 && genotype.alleles.iter().all(|a| !allele_bad(a))
}

fn normalized_pair(alleles: &[String]) -> Vec<String> {
    let mut pair = alleles.to_vec();
    pair.sort();
    pair
}

fn genotype_weight(genotype: &Genotype) -> Option<i64> {
    if let Some(quality) = genotype.quality {
        let rounded = quality.round().max(0.0) as i64;
        return Some(rounded.max(if genotype.genotype_likelihoods.iter().all(|v| *v == 0.0) && !genotype.genotype_likelihoods.is_empty() { 0 } else { 1 }));
    }
    if genotype.genotype_likelihoods.is_empty() { return Some(1); }
    let best = genotype.genotype_likelihoods.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if best.is_finite() { Some((best * 100.0).round().max(0.0) as i64) } else { Some(0) }
}

fn pair_inheritance(child: &[String], father: Option<&[String]>, mother: Option<&[String]>) -> bool {
    child.len() == 2 && (0..2).any(|from_father| {
        let from_mother = from_father ^ 1;
        father.is_none_or(|f| f.contains(&child[from_father]))
            && mother.is_none_or(|m| m.contains(&child[from_mother]))
    })
}

fn calls_after(observations: &[&Genotype], removed: &BTreeSet<String>) -> BTreeSet<Vec<String>> {
    observations.iter()
        .filter(|g| !removed.contains(&g.id))
        .filter(|g| !g.missing && g.alleles.len() == 2)
        .map(|g| normalized_pair(&g.alleles))
        .collect()
}

fn minimal_repairs<F>(ids: &[String], predicate: F) -> Vec<Vec<String>>
where F: Fn(&BTreeSet<String>) -> bool {
    let set: BTreeSet<String> = ids.iter().cloned().collect();
    if predicate(&set) { return vec![]; }
    let mut result: BTreeSet<Vec<String>> = BTreeSet::new();
    let n = ids.len().min(12);
    for mask in 0u32..(1u32 << n) {
        if mask.count_ones() as usize > ids.len().min(4) { continue; }
        let removed: BTreeSet<String> = ids.iter().enumerate()
            .filter(|(i, _)| (mask >> i) & 1 == 1)
            .map(|(_, id)| id.clone())
            .collect();
        if predicate(&removed) {
            result.insert(removed.into_iter().collect());
        }
    }
    if result.is_empty() {
        let all: Vec<String> = ids.to_vec();
        if predicate(&all.iter().cloned().collect()) { return vec![all]; }
        return vec![];
    }
    let min_len = result.iter().map(Vec::len).min().unwrap_or(0);
    result.into_iter().filter(|item| item.len() == min_len).collect()
}

fn fingerprint_str(values: &[String]) -> String {
    format!("{:016x}", fnv1a_64(&values.join("|")))
}

fn fingerprint_phases(phases: &BTreeMap<String, i32>) -> String {
    let text = phases.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("|");
    format!("{:016x}", fnv1a_64(&text))
}

fn fnv1a_64(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn optional_matrix<'a>(values: &'a [&'a Genotype], expected: bool) -> Vec<Option<&'a &'a Genotype>> {
    if values.is_empty() {
        if expected { Vec::new() } else { vec![None] }
    } else {
        values.iter().map(Some).collect()
    }
}
