pub mod engine;
pub mod model;
pub mod store;
pub mod web;

pub mod api;

use engine::{compute, EngineInput, StateView};
use model::*;
use std::sync::Mutex;
use store::Store;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("输入版本不匹配：期望 {expected}，当前 {current}（基于旧版本的并发裁定被拒绝）")]
    VersionMismatch { expected: i64, current: i64 },
    #[error("裁定无效：{0}")]
    Invalid(String),
    #[error("数据库错误：{0}")]
    Db(#[from] rusqlite::Error),
}

pub struct App {
    pub store: Mutex<Store>,
}

impl App {
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        Ok(App { store: Mutex::new(Store::open(path)?) })
    }

    pub fn import(&self, bundle: &ImportBundle) -> i64 {
        self.store.lock().unwrap().import(bundle)
    }

    pub fn view(&self, branch: &str) -> StateView {
        let store = self.store.lock().unwrap();
        let mut view = self::engine::compute(&EngineInput {
            samples: store.samples(),
            relationships: store.relationships(),
            variants: store.variants(),
            observations: store.observations(),
            read_links: store.read_links(),
            events: store.events(branch),
            input_version: store.input_version(),
        });
        view.branch = branch.to_string();
        view.branches = store.branches();
        view
    }

    /// Apply an adjudication decision. `expected_version` pins the input
    /// version the reviewer saw; stale concurrent decisions are rejected.
    pub fn decide(
        &self,
        branch: &str,
        expected_version: i64,
        decision: Decision,
    ) -> Result<i64, AppError> {
        let store = self.store.lock().unwrap();
        let current = store.input_version();
        if current != expected_version {
            return Err(AppError::VersionMismatch { expected: expected_version, current });
        }
        validate(&decision, &store, branch)?;
        Ok(store.append_event(branch, current, &decision))
    }

    pub fn rollback(&self, branch: &str) -> Option<DecisionEvent> {
        self.store.lock().unwrap().rollback(branch)
    }

    pub fn fork(&self, from: &str, to: &str, at_seq: Option<i64>) {
        self.store.lock().unwrap().fork_branch(from, to, at_seq)
    }

    pub fn branches(&self) -> Vec<String> {
        self.store.lock().unwrap().branches()
    }

    pub fn export(&self, branch: &str) -> ExportBundle {
        let store = self.store.lock().unwrap();
        ExportBundle {
            input_version: store.input_version(),
            pinned_batches: store.pinned_batches(),
            events: store.events(branch),
        }
    }

    /// Replay pinned batches and adjudication events onto a fresh database,
    /// reproducing the review state step by step after a restart.
    pub fn replay(app: &App, snapshot: &ExportBundle, raw: &ImportBundle) {
        let mut store = app.store.lock().unwrap();
        store.conn.execute_batch("DELETE FROM events").unwrap();
        // Raw observations are replayed as the pinned input versions.
        let _ = store.import(raw);
        for ev in &snapshot.events {
            store.append_event(&ev.branch, snapshot.input_version, &ev.decision);
        }
    }
}

fn validate(decision: &Decision, store: &Store, branch: &str) -> Result<(), AppError> {
    match decision {
        Decision::AcceptCandidate { block_id, candidate_index } => {
            // Candidate validity is checked against the live computed view.
            let view = compute(&EngineInput {
                samples: store.samples(),
                relationships: store.relationships(),
                variants: store.variants(),
                observations: store.observations(),
                read_links: store.read_links(),
                events: store.events(branch),
                input_version: store.input_version(),
            });
            let block = view
                .blocks
                .iter()
                .find(|b| &b.id == block_id)
                .ok_or_else(|| AppError::Invalid(format!("未知 block {}", block_id)))?;
            if candidate_index >= &block.candidates.len() {
                return Err(AppError::Invalid(format!(
                    "候选下标 {} 超出范围（共 {} 个）",
                    candidate_index,
                    block.candidates.len()
                )));
            }
            Ok(())
        }
        Decision::MarkRelationship { relationship_id, .. } => {
            if store.relationships().iter().any(|r| r.id == *relationship_id) {
                Ok(())
            } else {
                Err(AppError::Invalid(format!("未知关系 {}", relationship_id)))
            }
        }
        Decision::LockPhase { sample, chrom, from_pos, to_pos, .. } => {
            if !store.samples().iter().any(|s| s.id == *sample) {
                return Err(AppError::Invalid(format!("未知样本 {}", sample)));
            }
            if !store.variants().iter().any(|v| &v.chrom == chrom) {
                return Err(AppError::Invalid(format!("未知染色体 {}", chrom)));
            }
            if from_pos > to_pos {
                return Err(AppError::Invalid("区间起点不能大于终点".to_string()));
            }
            Ok(())
        }
        Decision::RetractReadLink { link_id } => {
            if store.read_links().iter().any(|l| l.id == *link_id && !l.retracted) {
                Ok(())
            } else {
                Err(AppError::Invalid(format!("读段连接 {} 不存在或已撤回", link_id)))
            }
        }
    }
}
