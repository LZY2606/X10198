use crate::model::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Candidate enumeration budget per phase block: at most this many candidate
/// phase assignments are kept. Once reached, the block is flagged and the UI
/// must never present the result as the unique solution.
pub const CANDIDATE_BUDGET: usize = 64;

/// Phred-style quality thresholds for raw genotype calls.
pub const HET_QUALITY_FLOOR: f64 = 20.0;
pub const LOCK_WEIGHT: f64 = 1000.0;

#[derive(Debug, Clone)]
pub struct Call {
    pub observation_id: i64,
    pub unordered: (usize, usize),
    pub is_het: bool,
    pub quality: f64,
}

/// Inputs to the phasing engine (already loaded from SQLite).
pub struct EngineInput {
    pub samples: Vec<Sample>,
    pub relationships: Vec<Relationship>,
    pub variants: Vec<Variant>,
    pub observations: Vec<Observation>,
    pub read_links: Vec<ReadLink>,
    pub events: Vec<DecisionEvent>,
    pub input_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixCell {
    pub observation_id: Option<i64>,
    pub genotype: String,
    pub missing: bool,
    pub low_quality_het: bool,
    pub ignored: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateView {
    pub input_version: i64,
    pub branch: String,
    pub samples: Vec<Sample>,
    pub relationships: Vec<Relationship>,
    pub variants: Vec<Variant>,
    pub matrix: BTreeMap<i64, BTreeMap<i64, MatrixCell>>,
    pub blocks: Vec<PhaseBlock>,
    pub conflicts: Vec<Conflict>,
    pub events: Vec<DecisionEvent>,
    pub branches: Vec<String>,
    pub ignored_observations: BTreeSet<i64>,
}

fn genotype_index(i: usize, j: usize) -> usize {
    let (i, j) = (i.min(j), i.max(j));
    j * (j + 1) / 2 + i
}

/// Normalise GLs (probabilities or phred-like) and pick the MAP genotype.
fn call_genotype(obs: &Observation) -> Option<Call> {
    let gls = &obs.gls;
    if gls.is_empty() {
        return None;
    }
    // Accept either plain probabilities or phred-scaled log likelihoods:
    // normalise into probabilities when values are not already probabilities.
    let probs: Vec<f64> = if gls.iter().any(|&x| x < 0.0) {
        let max = gls.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let raw: Vec<f64> = gls.iter().map(|x| (x - max).exp()).collect();
        let sum: f64 = raw.iter().sum();
        let n = raw.len();
        raw.into_iter()
            .map(move |x| if sum > 0.0 { x / sum } else { 1.0 / n as f64 })
            .collect()
    } else {
        let sum: f64 = gls.iter().sum();
        if sum <= 0.0 {
            return None;
        }
        gls.iter().map(|x| x / sum).collect()
    };
    let mut best = 0usize;
    for (i, p) in probs.iter().enumerate() {
        if *p > probs[best] {
            best = i;
        }
    }
    // Recover the unordered allele pair from the genotype index.
    let mut j = 0usize;
    while genotype_index(0, j + 1) <= best {
        j += 1;
    }
    let i = best - j * (j + 1) / 2;
    Some(Call {
        observation_id: obs.id,
        unordered: (i, j),
        is_het: i != j,
        quality: obs.quality,
    })
}

fn genotype_label(call: Option<&Call>, variant: &Variant) -> String {
    match call {
        None => ".".to_string(),
        Some(c) => {
            let a = variant.alleles.get(c.unordered.0).map(String::as_str).unwrap_or("?");
            let b = variant.alleles.get(c.unordered.1).map(String::as_str).unwrap_or("?");
            format!("{}/{}", a, b)
        }
    }
}

fn mendelian_consistent(child: (usize, usize), father: Option<(usize, usize)>,
                        mother: Option<(usize, usize)>) -> bool {
    let (ca, cb) = child;
    let can_be_from = |parent: (usize, usize), allele: usize| parent.0 == allele || parent.1 == allele;
    let options = [(ca, cb), (cb, ca)];
    options.iter().any(|&(pat, mat)| {
        father.map_or(true, |f| can_be_from(f, pat))
            && mother.map_or(true, |m| can_be_from(m, mat))
    })
}

/// Smallest observation subset whose exclusion makes a trio consistent.
/// Returns every minimum subset (same size); the engine records the first
/// (lowest-quality observation) as the recommended quarantine set.
fn minimal_ignore_set(
    trio: [Option<(i64, (usize, usize), f64)>; 3],
    _child_gt: (usize, usize),
) -> Vec<Vec<i64>> {
    let present: Vec<(i64, (usize, usize), f64)> = trio.iter().flatten().cloned().collect();
    let mut answers: Vec<Vec<i64>> = Vec::new();
    'outer: for size in 1..=present.len() {
        let mut indices: Vec<usize> = (0..size).collect();
        loop {
            let removed: BTreeSet<i64> = indices.iter().map(|&i| present[i].0).collect();
            let keep = |slot: usize| -> Option<(usize, usize)> {
                trio[slot].and_then(|x| if removed.contains(&x.0) { None } else { Some(x.1) })
            };
            // Removing the child observation drops the constraint entirely.
            let consistent = keep(0)
                .map(|gt| mendelian_consistent(gt, keep(1), keep(2)))
                .unwrap_or(true);
            if consistent {
                let mut ids: Vec<i64> = removed.into_iter().collect();
                ids.sort_by_key(|id| {
                    trio.iter()
                        .flatten()
                        .find(|p| p.0 == *id)
                        .map(|p| p.2 as i64)
                        .unwrap_or(0)
                });
                answers.push(ids);
            }
            if !next_combination(&mut indices, present.len()) {
                break;
            }
        }
        if !answers.is_empty() {
            break 'outer;
        }
    }
    answers
}

fn next_combination(comb: &mut Vec<usize>, n: usize) -> bool {
    let k = comb.len();
    if k == 0 || n < k {
        return false;
    }
    let mut i = k;
    loop {
        i -= 1;
        if comb[i] as usize + k - i < n {
            comb[i] += 1;
            for j in (i + 1)..k {
                comb[j] = comb[j - 1] + 1;
            }
            return true;
        }
        if i == 0 {
            return false;
        }
    }
}

fn slot_call(
    calls: &HashMap<(i64, i64), Call>,
    sample: i64,
    variant: i64,
) -> Option<(i64, (usize, usize), f64)> {
    calls.get(&(sample, variant)).map(|c| (c.observation_id, c.unordered, c.quality))
}

#[allow(clippy::too_many_arguments)]
fn check_trio_site(
    variant: &Variant,
    child: i64,
    fr: Option<&Relationship>,
    mr: Option<&Relationship>,
    rel_status: &BTreeMap<i64, RelStatus>,
    calls: &HashMap<(i64, i64), Call>,
    lowq_het: &BTreeSet<i64>,
    conflicts: &mut Vec<Conflict>,
    ignored: &mut BTreeSet<i64>,
) {
    let cc = match slot_call(calls, child, variant.id) {
        Some(c) => c,
        None => return, // missing genotype already reported
    };
    if lowq_het.contains(&cc.0) {
        return;
    }
    let fc = fr.and_then(|r| slot_call(calls, r.parent, variant.id));
    let mc = mr.and_then(|r| slot_call(calls, r.parent, variant.id));

    // Only confirmed relationships constrain Mendelian transmission.
    let fc_use = fr.filter(|r| rel_status[&r.id] == RelStatus::Confirmed)
        .and(fc);
    let mc_use = mr.filter(|r| rel_status[&r.id] == RelStatus::Confirmed)
        .and(mc);

    let trio = [Some(cc), fc_use, mc_use];
    if mendelian_consistent(cc.1, fc_use.map(|x| x.1), mc_use.map(|x| x.1)) {
        return;
    }

    // De novo: every allele of the child is absent from both called parents.
    let de_novo = [cc.1 .0, cc.1 .1].iter().all(|a| {
        fc_use.map_or(true, |f| f.1 .0 != *a && f.1 .1 != *a)
            && mc_use.map_or(true, |m| m.1 .0 != *a && m.1 .1 != *a)
    });

    let sets = minimal_ignore_set(trio, cc.1);
    let recommended = sets.first().cloned().unwrap_or_default();
    let evidence: Vec<i64> = trio.iter().flatten().map(|x| x.0).collect();
    for id in &recommended {
        ignored.insert(*id);
    }
    let mut alternatives = sets.clone();
    alternatives.sort();
    let detail = if de_novo {
        format!(
            "样本 {} 在变异 {} 出现 de novo 候选；隔离观测 {:?} 可使约束恢复一致（共 {} 个最小集合）",
            child, variant.id, recommended, sets.len()
        )
    } else {
        format!(
            "样本 {} 在变异 {} 出现 Mendelian 不一致；最小忽略集合 {:?} 可恢复一致（共 {} 个）",
            child, variant.id, recommended, sets.len()
        )
    };
    conflicts.push(Conflict {
        kind: if de_novo { ConflictKind::DeNovoCandidate } else { ConflictKind::MendelianInconsistent },
        sample: Some(child),
        variant: Some(variant.id),
        detail,
        evidence,
        minimal_ignore: recommended,
    });
}

fn paternal_allele_index(
    variant: &Variant,
    child_gt: (usize, usize),
    father_gt: Option<(usize, usize)>,
) -> Option<usize> {
    // Only biallelic heterozygous children are phasable by transmission.
    if variant.alleles.len() != 2 || child_gt.0 == child_gt.1 {
        return None;
    }
    let (a, b) = child_gt;
    match father_gt {
        Some((p, q)) if p == q => {
            if p == a {
                Some(a)
            } else if p == b {
                Some(b)
            } else {
                None
            }
        }
        _ => None,
    }
}

struct Dsu {
    parent: BTreeMap<i64, i64>,
}
impl Dsu {
    fn add(&mut self, x: i64) {
        self.parent.entry(x).or_insert(x);
    }
    fn find(&mut self, x: i64) -> i64 {
        let p = self.parent[&x];
        if p == x {
            return x;
        }
        let r = self.find(p);
        self.parent.insert(x, r);
        r
    }
    fn union(&mut self, a: i64, b: i64) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent.insert(ra, rb);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_blocks(
    input: &EngineInput,
    trios: &BTreeMap<i64, (Option<&Relationship>, Option<&Relationship>)>,
    rel_status: &BTreeMap<i64, RelStatus>,
    calls: &HashMap<(i64, i64), Call>,
    ignored: &BTreeSet<i64>,
    retracted: &BTreeSet<i64>,
    locked: &BTreeMap<(i64, i64, i64), bool>,
    accepted: &BTreeMap<String, usize>,
    edges: &mut Vec<PhaseEdge>,
    _conflicts: &[Conflict],
) -> Vec<PhaseBlock> {
    let var_index: HashMap<i64, &Variant> = input.variants.iter().map(|v| (v.id, v)).collect();

    // ---- Transmission edges (parent -> child haplotype continuity).
    for (child, (fr, mr)) in trios {
        let father_gt = |vid: i64| -> Option<(usize, usize)> {
            fr.filter(|r| rel_status[&r.id] == RelStatus::Confirmed)
                .and_then(|r| calls.get(&(r.parent, vid)).map(|c| c.unordered))
        };
        let _ = mr;
        let mut informative: Vec<(&Variant, usize)> = Vec::new();
        for v in &input.variants {
            if let Some(cc) = calls.get(&(*child, v.id)) {
                if ignored.contains(&cc.observation_id) {
                    continue;
                }
                if let Some(pi) = paternal_allele_index(v, cc.unordered, father_gt(v.id)) {
                    informative.push((v, pi));
                }
            }
        }
        informative.sort_by(|a, b| a.0.chrom.cmp(&b.0.chrom).then(a.0.pos.cmp(&b.0.pos)));
        for chrom_group in group_by_chrom(informative) {
            for win in chrom_group.windows(2) {
                let (v1, p1) = win[0];
                let (v2, p2) = win[1];
                // allele-0 (reference) on paternal haplotype at both sites?
                let same = p1 == 0 && p2 == 0 || p1 != 0 && p2 != 0;
                let support: Vec<i64> = [
                    calls.get(&(*child, v1.id)).map(|c| c.observation_id),
                    calls.get(&(*child, v2.id)).map(|c| c.observation_id),
                    fr.and_then(|r| calls.get(&(r.parent, v1.id)).map(|c| c.observation_id)),
                    fr.and_then(|r| calls.get(&(r.parent, v2.id)).map(|c| c.observation_id)),
                ]
                .into_iter()
                .flatten()
                .collect();
                edges.push(PhaseEdge {
                    sample: *child,
                    var_a: v1.id,
                    var_b: v2.id,
                    same,
                    weight: 1.0,
                    source: EdgeSource::Transmission,
                    support,
                });
            }
        }
    }

    // ---- Read-link edges (non-retracted only).
    for link in &input.read_links {
        if link.retracted || retracted.contains(&link.id) {
            continue;
        }
        // A read link on a sample only counts if both sites are called hets.
        let ca = calls.get(&(link.sample, link.var_a));
        let cb = calls.get(&(link.sample, link.var_b));
        match (ca, cb) {
            (Some(ca), Some(cb)) if ca.is_het && cb.is_het => {
                if ignored.contains(&ca.observation_id) || ignored.contains(&cb.observation_id) {
                    continue;
                }
                edges.push(PhaseEdge {
                    sample: link.sample,
                    var_a: link.var_a,
                    var_b: link.var_b,
                    same: link.same_haplotype,
                    weight: link.weight,
                    source: EdgeSource::ReadLink,
                    support: vec![link.id, ca.observation_id, cb.observation_id],
                });
            }
            _ => {}
        }
    }

    // ---- Locked local-phase decisions become hard edges.
    for ((sample, va, vb), same) in locked {
        if calls.get(&(*sample, *va)).is_some() && calls.get(&(*sample, *vb)).is_some() {
            edges.push(PhaseEdge {
                sample: *sample,
                var_a: *va,
                var_b: *vb,
                same: *same,
                weight: LOCK_WEIGHT,
                source: EdgeSource::Locked,
                support: Vec::new(),
            });
        }
    }

    // ---- Connected components per sample+chromosome, starting from
    // transmission edges and merging through read-link bridges.
    let mut blocks: Vec<PhaseBlock> = Vec::new();
    let mut chroms: Vec<String> = input.variants.iter().map(|v| v.chrom.clone()).collect();
    chroms.sort();
    chroms.dedup();
    for sample in &input.samples {
        for chrom in &chroms {
            let mut dsu = Dsu { parent: BTreeMap::new() };
            let mut node_set: BTreeSet<i64> = BTreeSet::new();
            let node_edges: Vec<&PhaseEdge> = edges
                .iter()
                .filter(|e| {
                    e.sample == sample.id
                        && var_index[&e.var_a].chrom == *chrom
                        && var_index[&e.var_b].chrom == *chrom
                })
                .collect();
            // Stage 1: transmission/locked components form the seeds.
            let has_non_read = |v: i64| {
                node_edges
                    .iter()
                    .any(|x| x.source != EdgeSource::ReadLink && (x.var_a == v || x.var_b == v))
            };
            for e in &node_edges {
                dsu.add(e.var_a);
                dsu.add(e.var_b);
                if e.source != EdgeSource::ReadLink {
                    dsu.union(e.var_a, e.var_b);
                }
            }
            // Stage 2: read links grow/merge components in position order.
            // A read link that joins two previously separate components is a
            // bridge; its id is recorded so retraction can split the region.
            let mut bridge_edges: BTreeSet<i64> = BTreeSet::new();
            let mut ordered: Vec<&&PhaseEdge> = node_edges
                .iter()
                .filter(|e| e.source == EdgeSource::ReadLink)
                .collect();
            ordered.sort_by_key(|e| {
                var_index[&e.var_a].pos + var_index[&e.var_b].pos
            });
            for e in ordered {
                dsu.add(e.var_a);
                dsu.add(e.var_b);
                if dsu.find(e.var_a) != dsu.find(e.var_b) {
                    // Only a link into a transmission-seeded component is a
                    // merge of two independently derived phase blocks.
                    if has_non_read(e.var_a) || has_non_read(e.var_b) {
                        if let Some(link_id) = e.support.first() {
                        bridge_edges.insert(*link_id);
                        }
                    }
                    dsu.union(e.var_a, e.var_b);
                }
            }
            for e in &node_edges {
                node_set.insert(e.var_a);
                node_set.insert(e.var_b);
            }
            let mut comps: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
            for n in &node_set {
                comps.entry(dsu.find(*n)).or_default().push(*n);
            }
            for (_, mut members) in comps {
                members.sort_by_key(|id| var_index[id].pos);
                let block_edges: Vec<PhaseEdge> = node_edges
                    .iter()
                    .filter(|e| members.contains(&e.var_a))
                    .map(|e| (*e).clone())
                    .collect();
                let bridge_evidence: Vec<i64> = block_edges
                    .iter()
                    .filter(|e| e.source == EdgeSource::ReadLink)
                    .filter_map(|e| e.support.first().copied())
                    .filter(|id| bridge_edges.contains(id))
                    .collect();
                let id = format!(
                    "B-{}-{}-{}",
                    sample.id, chrom, members.first().copied().unwrap_or(0)
                );
                let (candidates, budget_hit) =
                    enumerate_candidates(&members, &block_edges, accepted.get(&id));
                blocks.push(PhaseBlock {
                    id,
                    sample: sample.id,
                    chrom: chrom.clone(),
                    start_pos: var_index[members.first().unwrap()].pos,
                    end_pos: var_index[members.last().unwrap()].pos,
                    variant_ids: members,
                    bridge_evidence,
                    candidates,
                    budget_hit,
                });
            }
        }
    }
    blocks.sort_by(|a, b| a.sample.cmp(&b.sample).then(a.start_pos.cmp(&b.start_pos)));
    blocks
}

fn group_by_chrom<'a>(items: Vec<(&'a Variant, usize)>) -> Vec<Vec<(&'a Variant, usize)>> {
    let mut groups: BTreeMap<String, Vec<(&'a Variant, usize)>> = BTreeMap::new();
    for item in items {
        groups.entry(item.0.chrom.clone()).or_default().push(item);
    }
    groups.into_values().collect()
}

fn edge_satisfied(edge: &PhaseEdge, pos: &HashMap<i64, usize>, orient: &[bool]) -> bool {
    let pa = match pos.get(&edge.var_a) { Some(p) if *p < orient.len() => *p, _ => return false };
    let pb = match pos.get(&edge.var_b) { Some(p) if *p < orient.len() => *p, _ => return false };
    let oa = orient[pa];
    let ob = orient[pb];
    (oa == ob) == edge.same
}

fn edge_ready(edge: &PhaseEdge, pos: &HashMap<i64, usize>, len: usize) -> bool {
    matches!(pos.get(&edge.var_a), Some(p) if *p < len)
        && matches!(pos.get(&edge.var_b), Some(p) if *p < len)
}

fn score_candidate(
    orient: &[bool],
    pos: &HashMap<i64, usize>,
    edges: &[PhaseEdge],
) -> (f64, Vec<PhaseEdge>, Vec<String>) {
    let mut score = 0.0;
    let mut supporting = Vec::new();
    let mut ignored = Vec::new();
    for e in edges {
        if !edge_ready(e, pos, orient.len()) {
            continue;
        }
        if edge_satisfied(e, pos, orient) {
            score += e.weight;
            supporting.push(e.clone());
        } else {
            ignored.push(format!(
                "{} 证据 {:?} 与该候选冲突（权重 {}）",
                source_label(e.source),
                e.support,
                e.weight
            ));
        }
    }
    (score, supporting, ignored)
}

fn source_label(s: EdgeSource) -> &'static str {
    match s {
        EdgeSource::Transmission => "传递",
        EdgeSource::ReadLink => "读段",
        EdgeSource::Locked => "锁定",
    }
}

fn enumerate_candidates(
    members: &[i64],
    edges: &[PhaseEdge],
    accepted_index: Option<&usize>,
) -> (Vec<Candidate>, bool) {
    let n = members.len();
    let pos: HashMap<i64, usize> =
        members.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    // Orientation of the first variant is fixed to allele-0 -> haplotype A.
    // This quotients out the global parent-origin swap (paternal/maternal
    // haplotypes relabelled), which is observationally equivalent.
    let mut raws: Vec<Vec<bool>> = Vec::new();
    let mut budget_hit = false;
    let free = n.saturating_sub(1);
    let exact_cap = 1u32 << 6; // exhaustive search only up to the budget
    if (1u32 << free) <= exact_cap {
        for mask in 0..(1u32 << free) {
            let mut v = vec![true];
            for i in 0..free {
                v.push(mask & (1 << i) != 0);
            }
            raws.push(v);
        }
    } else {
        // Candidate budget exhausted: beam search keeps the best budget of
        // assignments; completeness is lost, so the caller must not present
        // the result as the unique solution.
        budget_hit = true;
        let mut beam: Vec<Vec<bool>> = vec![vec![true]];
        for _ in 0..free {
            let mut next: Vec<Vec<bool>> = Vec::new();
            for cand in &beam {
                let mut a = cand.clone();
                a.push(true);
                let mut b = cand.clone();
                b.push(false);
                next.push(a);
                next.push(b);
            }
            next.sort_by(|x, y| {
                let sx = score_candidate(x, &pos, edges).0;
                let sy = score_candidate(y, &pos, edges).0;
                sy.partial_cmp(&sx).unwrap_or(std::cmp::Ordering::Equal)
            });
            next.dedup();
            next.truncate(CANDIDATE_BUDGET);
            beam = next;
        }
        raws = beam;
    }

    let mut scored: Vec<(f64, Vec<bool>, Vec<PhaseEdge>, Vec<String>)> = raws
        .into_iter()
        .map(|orient| {
            let (s, sup, ign) = score_candidate(&orient, &pos, edges);
            (s, orient, sup, ign)
        })
        .collect();
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
    });
    let best = scored.first().map(|x| x.0).unwrap_or(0.0);
    // Every top-scoring (tied) candidate must be retained.  When the exact
    // enumeration yields more ties than the budget, keep them all for
    // correctness but flag the block so it is never treated as unique.
    let top: Vec<_> = scored
        .into_iter()
        .filter(|x| (x.0 - best).abs() < 1e-9)
        .collect();
    if !budget_hit && top.len() > CANDIDATE_BUDGET {
        budget_hit = true;
    }
    let top: Vec<_> = top.into_iter().take(CANDIDATE_BUDGET).collect();
    if top.len() > CANDIDATE_BUDGET {
        budget_hit = true;
    }
    let candidates = top
        .into_iter()
        .enumerate()
        .map(|(i, (score, orientation, supporting, mut ignored))| {
            if edges.is_empty() {
                ignored.push("块内无支持证据".to_string());
            }
            Candidate {
                index: i,
                score,
                orientation,
                supporting,
                ignored,
                accepted: accepted_index == Some(&i),
            }
        })
        .collect();
    (candidates, budget_hit)
}

pub fn compute(input: &EngineInput) -> StateView {
    // --- Apply adjudication history: relationship revisions, retractions
    // are already reflected in rows; derive effective state from events.
    let mut rel_status: BTreeMap<i64, RelStatus> = BTreeMap::new();
    for r in &input.relationships {
        rel_status.insert(r.id, r.status);
    }
    let mut retracted: BTreeSet<i64> = BTreeSet::new();
    let mut accepted: BTreeMap<String, usize> = BTreeMap::new();
    // locked edges keyed (sample, var_a, var_b) -> same?
    let mut locked: BTreeMap<(i64, i64, i64), bool> = BTreeMap::new();
    for ev in &input.events {
        match &ev.decision {
            Decision::MarkRelationship { relationship_id, status } => {
                rel_status.insert(*relationship_id, *status);
            }
            Decision::RetractReadLink { link_id } => {
                retracted.insert(*link_id);
            }
            Decision::AcceptCandidate { block_id, candidate_index } => {
                accepted.insert(block_id.clone(), *candidate_index);
            }
            Decision::LockPhase { sample, chrom, from_pos, to_pos, orientation } => {
                let mut ids: Vec<&Variant> = input.variants.iter()
                    .filter(|v| v.chrom == *chrom && v.pos >= *from_pos && v.pos <= *to_pos)
                    .collect();
                ids.sort_by_key(|v| v.pos);
                let orient: BTreeMap<i64, bool> = orientation.iter().cloned().collect();
                for win in ids.windows(2) {
                    if let (Some(oa), Some(ob)) = (orient.get(&win[0].id), orient.get(&win[1].id)) {
                        locked.insert((*sample, win[0].id, win[1].id), oa == ob);
                    }
                }
            }
        }
    }

    // --- Latest observation wins per (sample, variant).
    let mut latest_obs: BTreeMap<(i64, i64), &Observation> = BTreeMap::new();
    for o in &input.observations {
        latest_obs.entry((o.sample, o.variant))
            .and_modify(|cur| if o.version > cur.version { *cur = o; })
            .or_insert(o);
    }
    let mut calls: HashMap<(i64, i64), Call> = HashMap::new();
    let mut missing_obs: BTreeSet<i64> = BTreeSet::new();
    let mut lowq_het: BTreeSet<i64> = BTreeSet::new();
    for ((s, v), obs) in &latest_obs {
        match call_genotype(obs) {
            None => { missing_obs.insert(obs.id); }
            Some(c) => {
                if c.is_het && c.quality < HET_QUALITY_FLOOR {
                    lowq_het.insert(c.observation_id);
                }
                calls.insert((*s, *v), c);
            }
        }
    }

    let mut conflicts: Vec<Conflict> = Vec::new();
    let mut ignored: BTreeSet<i64> = BTreeSet::new();

    // --- Duplicate samples: identical anonymous label on distinct ids.
    let mut labels: HashMap<&str, Vec<i64>> = HashMap::new();
    for s in &input.samples {
        labels.entry(&s.anon_label).or_default().push(s.id);
    }
    for (label, ids) in labels {
        if ids.len() > 1 {
            conflicts.push(Conflict {
                kind: ConflictKind::DuplicateSample,
                sample: None,
                variant: None,
                detail: format!("匿名标识 {} 对应多个样本: {:?}", label, ids),
                evidence: Vec::new(),
                minimal_ignore: Vec::new(),
            });
        }
    }

    // --- Missing / low-quality conflicts.
    let obs_index: HashMap<i64, &Observation> =
        input.observations.iter().map(|o| (o.id, o)).collect();
    for id in &missing_obs {
        let o = obs_index[id];
        conflicts.push(Conflict {
            kind: ConflictKind::MissingGenotype,
            sample: Some(o.sample),
            variant: Some(o.variant),
            detail: format!("样本 {} 在变异 {} 的基因型缺失", o.sample, o.variant),
            evidence: vec![*id],
            minimal_ignore: Vec::new(),
        });
    }
    for id in &lowq_het {
        let o = obs_index[id];
        ignored.insert(*id);
        conflicts.push(Conflict {
            kind: ConflictKind::LowQualityHet,
            sample: Some(o.sample),
            variant: Some(o.variant),
            detail: format!("样本 {} 在变异 {} 的杂合低于质量阈值 {}",
                o.sample, o.variant, HET_QUALITY_FLOOR),
            evidence: vec![*id],
            minimal_ignore: Vec::new(),
        });
    }

    // --- Trio Mendelian / de novo checks per variant.
    let var_by_id: HashMap<i64, &Variant> = input.variants.iter().map(|v| (v.id, v)).collect();

    // One trio per child: father relationship + optional mother relationship.
    let mut trios: BTreeMap<i64, (Option<&Relationship>, Option<&Relationship>)> = BTreeMap::new();
    for r in &input.relationships {
        let entry = trios.entry(r.child).or_insert((None, None));
        match r.kind {
            ParentKind::Father => entry.0 = Some(r),
            ParentKind::Mother => entry.1 = Some(r),
        }
    }

    // Uncertain parentage conflicts are global to the relationship.
    for r in &input.relationships {
        if rel_status[&r.id] == RelStatus::ToConfirm {
            conflicts.push(Conflict {
                kind: ConflictKind::UncertainParentage,
                sample: Some(r.child),
                variant: None,
                detail: format!("关系 {} ({} -> {}) 待确认，不用于传递定相", r.id, r.parent, r.child),
                evidence: Vec::new(),
                minimal_ignore: Vec::new(),
            });
        }
    }

    for variant in &input.variants {
        for (child, (fr, mr)) in &trios {
            check_trio_site(
                variant, *child, *fr, *mr, &rel_status, &calls, &lowq_het,
                &mut conflicts, &mut ignored,
            );
        }
    }
    let _ = &obs_index;

    let mut edges: Vec<PhaseEdge> = Vec::new();
    let blocks = build_blocks(
        input, &trios, &rel_status, &calls, &ignored, &retracted, &locked,
        &accepted, &mut edges, &conflicts,
    );
    let _ = var_by_id;

    // --- Variant matrix.
    let mut matrix: BTreeMap<i64, BTreeMap<i64, MatrixCell>> = BTreeMap::new();
    for s in &input.samples {
        for v in &input.variants {
            let obs = latest_obs.get(&(s.id, v.id)).copied();
            let call = calls.get(&(s.id, v.id));
            matrix.entry(s.id).or_default().insert(v.id, MatrixCell {
                observation_id: obs.map(|o| o.id),
                genotype: genotype_label(call, v),
                missing: obs.map_or(true, |o| missing_obs.contains(&o.id))
                    || call.is_none(),
                low_quality_het: call.map_or(false, |c| lowq_het.contains(&c.observation_id)),
                ignored: obs.map_or(false, |o| ignored.contains(&o.id)),
            });
        }
    }

    StateView {
        input_version: input.input_version,
        branch: input.events.first().map(|e| e.branch.clone()).unwrap_or_else(|| "main".to_string()),
        samples: input.samples.clone(),
        relationships: {
            let mut rels = input.relationships.clone();
            for r in &mut rels {
                r.status = rel_status[&r.id];
            }
            rels
        },
        variants: input.variants.clone(),
        matrix,
        blocks,
        conflicts,
        events: input.events.clone(),
        branches: Vec::new(),
        ignored_observations: ignored,
    }
}
