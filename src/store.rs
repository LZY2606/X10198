use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::engine::{import_to_input, Analysis, Conflict};
use crate::model::*;

pub struct Store {
    db: Mutex<Connection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BranchRecord {
    pub id: String,
    pub name: String,
    pub version_id: String,
    pub parent_branch_id: Option<String>,
    pub cutoff_event_seq: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventRecord {
    pub seq: i64,
    pub id: String,
    pub branch_id: String,
    pub parent_branch_id: Option<String>,
    pub version_id: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportBundle {
    pub current_branch: BranchRecord,
    pub branches: Vec<BranchRecord>,
    pub pinned_version_id: String,
    pub analysis: Analysis,
    pub replay_events: Vec<EventRecord>,
    pub stale_events: Vec<EventRecord>,
}

impl Store {
    pub fn open(path: &str) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(|error| error.to_string())?;
        let store = Self {
            db: Mutex::new(connection),
        };
        store.init()?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Self, String> {
        let connection = Connection::open_in_memory().map_err(|error| error.to_string())?;
        let store = Self {
            db: Mutex::new(connection),
        };
        store.init()?;
        Ok(store)
    }

    fn init(&self) -> Result<(), String> {
        let db = self.db.lock().map_err(|error| error.to_string())?;
        db.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS import_versions (
                version_id TEXT PRIMARY KEY,
                note TEXT,
                candidate_budget INTEGER NOT NULL,
                raw_json TEXT NOT NULL,
                input_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS observations (
                id TEXT NOT NULL,
                version_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                batch TEXT,
                raw_json TEXT NOT NULL,
                seq INTEGER NOT NULL,
                PRIMARY KEY (version_id, id),
                FOREIGN KEY (version_id) REFERENCES import_versions(version_id)
            );
            CREATE TABLE IF NOT EXISTS branches (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                version_id TEXT NOT NULL,
                parent_branch_id TEXT,
                cutoff_event_seq INTEGER,
                created_at TEXT NOT NULL,
                UNIQUE(name),
                FOREIGN KEY (version_id) REFERENCES import_versions(version_id)
            );
            CREATE TABLE IF NOT EXISTS decision_events (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                id TEXT NOT NULL UNIQUE,
                branch_id TEXT NOT NULL,
                parent_branch_id TEXT,
                version_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .map_err(|error| error.to_string())
    }

    pub fn import(&self, payload: ImportPayload) -> Result<(String, Analysis), String> {
        let raw_json = serde_json::to_string_pretty(&payload).map_err(|error| error.to_string())?;
        let (input, conflicts) = import_to_input(payload);
        if self.version_exists(&input.version_id)? {
            return Err(format!("输入版本 {} 已存在", input.version_id));
        }
        let created_at = now_id();
        let version_id = input.version_id.clone();
        let input_json = serde_json::to_string(&input).map_err(|error| error.to_string())?;
        let branch_id = format!("branch-{}", crate::engine::stable_hash(&format!("{version_id}:{created_at}")));
        {
            let db = self.db.lock().map_err(|error| error.to_string())?;
            db.execute(
                "INSERT INTO import_versions(version_id, note, candidate_budget, raw_json, input_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    input.version_id,
                    input.note,
                    input.candidate_budget as i64,
                    raw_json,
                    input_json,
                    created_at
                ],
            )
            .map_err(|error| error.to_string())?;
            for (seq, observation) in input_observations_from_input(&input).iter().enumerate() {
                db.execute(
                    "INSERT INTO observations(id, version_id, kind, batch, raw_json, seq) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        observation.id,
                        version_id,
                        observation.kind,
                        observation.batch,
                        observation.raw_json,
                        seq as i64
                    ],
                )
                .map_err(|error| error.to_string())?;
            }
            db.execute(
                "INSERT INTO branches(id, name, version_id, parent_branch_id, cutoff_event_seq, created_at) VALUES (?1, ?2, ?3, NULL, NULL, ?4)",
                params![branch_id, version_id, version_id, created_at],
            )
            .map_err(|error| error.to_string())?;
            db.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES ('current_branch', ?1)",
                params![branch_id],
            )
            .map_err(|error| error.to_string())?;
        }
        let analysis = self.analysis(&branch_id)?;
        let _ = conflicts;
        Ok((branch_id, analysis))
    }

    fn version_exists(&self, version_id: &str) -> Result<bool, String> {
        let db = self.db.lock().map_err(|error| error.to_string())?;
        let mut statement = db
            .prepare("SELECT 1 FROM import_versions WHERE version_id = ?1")
            .map_err(|error| error.to_string())?;
        statement.exists(params![version_id]).map_err(|error| error.to_string())
    }

    pub fn current_branch_id(&self) -> Result<String, String> {
        let db = self.db.lock().map_err(|error| error.to_string())?;
        db.query_row("SELECT value FROM meta WHERE key = 'current_branch'", [], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|error| error.to_string())
    }

    pub fn set_current_branch(&self, branch_id: &str) -> Result<(), String> {
        let db = self.db.lock().map_err(|error| error.to_string())?;
        db.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES ('current_branch', ?1)",
            params![branch_id],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn current_analysis(&self) -> Result<(BranchRecord, Analysis, Vec<EventRecord>), String> {
        let branch_id = self.current_branch_id()?;
        let branch = self.branch(&branch_id)?;
        let (analysis, events) = self.analysis_with_events(&branch_id)?;
        Ok((branch, analysis, events))
    }

    pub fn analysis(&self, branch_id: &str) -> Result<Analysis, String> {
        Ok(self.analysis_with_events(branch_id)?.0)
    }

    fn analysis_with_events(&self, branch_id: &str) -> Result<(Analysis, Vec<EventRecord>), String> {
        let branch = self.branch(branch_id)?;
        let (input, import_conflicts) = self.load_version(&branch.version_id)?;
        let events = self.events_for_branch(branch_id)?;
        let overlay = overlay_from_events(&events);
        Ok((crate::engine::analyze(input, overlay, import_conflicts), events))
    }

    fn branch(&self, branch_id: &str) -> Result<BranchRecord, String> {
        let db = self.db.lock().map_err(|error| error.to_string())?;
        db.query_row(
            "SELECT id, name, version_id, parent_branch_id, cutoff_event_seq, created_at FROM branches WHERE id = ?1",
            params![branch_id],
            |row| {
                Ok(BranchRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    version_id: row.get(2)?,
                    parent_branch_id: row.get(3)?,
                    cutoff_event_seq: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(|error| error.to_string())
    }

    pub fn branches(&self) -> Result<Vec<BranchRecord>, String> {
        let db = self.db.lock().map_err(|error| error.to_string())?;
        let mut statement = db
            .prepare("SELECT id, name, version_id, parent_branch_id, cutoff_event_seq, created_at FROM branches ORDER BY created_at, id")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok(BranchRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    version_id: row.get(2)?,
                    parent_branch_id: row.get(3)?,
                    cutoff_event_seq: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.map(|row| row.map_err(|error| error.to_string())).collect()
    }

    fn load_version(&self, version_id: &str) -> Result<(AnalysisInput, Vec<Conflict>), String> {
        let db = self.db.lock().map_err(|error| error.to_string())?;
        let payload_json = db
            .query_row(
                "SELECT raw_json FROM import_versions WHERE version_id = ?1",
                params![version_id],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| error.to_string())?;
        let payload: ImportPayload = serde_json::from_str(&payload_json).map_err(|error| error.to_string())?;
        Ok(import_to_input(payload))
    }

    fn events_for_branch(&self, branch_id: &str) -> Result<Vec<EventRecord>, String> {
        let branch = self.branch(branch_id)?;
        let mut result = Vec::new();
        if let Some(parent_id) = &branch.parent_branch_id {
            result.extend(self.events_for_branch(parent_id)?);
        }
        let rows = {
            let db = self.db.lock().map_err(|error| error.to_string())?;
            let mut statement = db
            .prepare(
                "SELECT seq, id, branch_id, parent_branch_id, version_id, event_type, payload, created_at
                 FROM decision_events WHERE branch_id = ?1 ORDER BY seq",
            )
            .map_err(|error| error.to_string())?;
            let rows = statement
                .query_map(params![branch_id], event_from_row)
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>();
            rows.map_err(|error| error.to_string())?
        };
        result.extend(rows);
        if let Some(cutoff) = branch.cutoff_event_seq {
            result.retain(|event| event.seq < cutoff);
        }
        Ok(result)
    }

    pub fn all_events(&self) -> Result<Vec<EventRecord>, String> {
        let db = self.db.lock().map_err(|error| error.to_string())?;
        let mut statement = db
            .prepare(
                "SELECT seq, id, branch_id, parent_branch_id, version_id, event_type, payload, created_at
                 FROM decision_events ORDER BY seq",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement.query_map([], event_from_row).map_err(|error| error.to_string())?;
        rows.map(|row| row.map_err(|error| error.to_string())).collect()
    }

    pub fn record_decision(&self, decision: Decision) -> Result<EventRecord, String> {
        let branch_id = self.current_branch_id()?;
        let branch = self.branch(&branch_id)?;
        let event_type = match decision {
            Decision::AcceptCandidate { .. } => "accept_candidate",
            Decision::LockPhase { .. } => "lock_phase",
            Decision::MarkRelationshipUncertain { .. } => "mark_relationship_uncertain",
            Decision::ReviseRelationship { .. } => "revise_relationship",
            Decision::WithdrawReadLink { .. } => "withdraw_read_link",
        };
        let payload = serde_json::to_value(&decision).map_err(|error| error.to_string())?;
        let created_at = now_id();
        let id = format!(
            "event-{}",
            crate::engine::stable_hash(&format!("{}:{}:{}:{}", branch_id, branch.version_id, event_type, created_at))
        );
        {
            let db = self.db.lock().map_err(|error| error.to_string())?;
            db.execute(
                "INSERT INTO decision_events(id, branch_id, parent_branch_id, version_id, event_type, payload, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    branch_id,
                    branch.parent_branch_id,
                    branch.version_id,
                    event_type,
                    serde_json::to_string(&payload).map_err(|error| error.to_string())?,
                    created_at
                ],
            )
            .map_err(|error| error.to_string())?;
        }
        self.all_events()?
            .into_iter()
            .find(|event| event.id == id)
            .ok_or_else(|| "事件写入后无法读取".to_string())
    }

    pub fn rollback_current(&self) -> Result<BranchRecord, String> {
        let current_id = self.current_branch_id()?;
        let events = self.events_for_branch(&current_id)?;
        let cutoff = events
            .last()
            .map(|event| event.seq)
            .ok_or_else(|| "当前分支没有可回退的裁定".to_string())?;
        let branch = self.branch(&current_id)?;
        let new_id = format!("branch-rollback-{}", crate::engine::stable_hash(&format!("{current_id}:{cutoff}")));
        let created_at = now_id();
        {
            let db = self.db.lock().map_err(|error| error.to_string())?;
            db.execute(
                "INSERT INTO branches(id, name, version_id, parent_branch_id, cutoff_event_seq, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    new_id,
                    format!("{}@<{}", branch.name, cutoff),
                    branch.version_id,
                    current_id,
                    cutoff,
                    created_at
                ],
            )
            .map_err(|error| error.to_string())?;
            db.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES ('current_branch', ?1)",
                params![new_id],
            )
            .map_err(|error| error.to_string())?;
        }
        self.branch(&new_id)
    }

    pub fn fork_current(&self, name: &str) -> Result<BranchRecord, String> {
        let current_id = self.current_branch_id()?;
        let branch = self.branch(&current_id)?;
        let events = self.events_for_branch(&current_id)?;
        let cutoff = events.last().map(|event| event.seq + 1);
        let new_id = format!("branch-fork-{}", crate::engine::stable_hash(&format!("{name}:{current_id}:{}", now_id())));
        let created_at = now_id();
        {
            let db = self.db.lock().map_err(|error| error.to_string())?;
            db.execute(
                "INSERT INTO branches(id, name, version_id, parent_branch_id, cutoff_event_seq, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![new_id, name, branch.version_id, current_id, cutoff, created_at],
            )
            .map_err(|error| error.to_string())?;
            db.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES ('current_branch', ?1)",
                params![new_id],
            )
            .map_err(|error| error.to_string())?;
        }
        self.branch(&new_id)
    }

    pub fn export_current(&self) -> Result<ExportBundle, String> {
        let current_id = self.current_branch_id()?;
        let branch = self.branch(&current_id)?;
        let (analysis, replay_events) = self.analysis_with_events(&current_id)?;
        let replay_ids: std::collections::BTreeSet<i64> = replay_events.iter().map(|event| event.seq).collect();
        let stale_events = self
            .all_events()?
            .into_iter()
            .filter(|event| !replay_ids.contains(&event.seq))
            .collect();
        Ok(ExportBundle {
            branches: self.branches()?,
            pinned_version_id: branch.version_id.clone(),
            current_branch: branch,
            analysis,
            replay_events,
            stale_events,
        })
    }
}

#[derive(Serialize)]
struct StoredObservation {
    id: String,
    kind: String,
    batch: Option<String>,
    raw_json: String,
}

fn input_observations_from_input(input: &AnalysisInput) -> Vec<StoredObservation> {
    let mut items = Vec::new();
    for value in &input.samples {
        items.push(store_observation(&value.id, "sample", Some(value.batch.clone()), value));
    }
    for value in &input.relationships {
        items.push(store_observation(&value.id, "relationship", value.batch.clone(), value));
    }
    for value in &input.variants {
        items.push(store_observation(&value.id, "variant", Some(value.batch.clone()), value));
    }
    for value in &input.genotypes {
        items.push(store_observation(&value.id, "genotype", Some(value.batch.clone()), value));
    }
    for value in &input.read_links {
        items.push(store_observation(&value.id, "read_link", Some(value.batch.clone()), value));
    }
    items
}

fn store_observation<T: Serialize>(id: &str, kind: &str, batch: Option<String>, value: &T) -> StoredObservation {
    StoredObservation {
        id: id.to_string(),
        kind: kind.to_string(),
        batch,
        raw_json: serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string()),
    }
}

fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventRecord> {
    Ok(EventRecord {
        seq: row.get(0)?,
        id: row.get(1)?,
        branch_id: row.get(2)?,
        parent_branch_id: row.get(3)?,
        version_id: row.get(4)?,
        event_type: row.get(5)?,
        payload: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or(serde_json::Value::Null),
        created_at: row.get(7)?,
    })
}

fn overlay_from_events(events: &[EventRecord]) -> DecisionOverlay {
    let mut overlay = DecisionOverlay::default();
    for event in events {
        if let Ok(decision) = serde_json::from_value::<Decision>(event.payload.clone()) {
            overlay.decisions.push(decision);
        }
    }
    overlay
}

fn now_id() -> String {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos())
}
