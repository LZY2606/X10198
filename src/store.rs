use crate::analysis::{self, Analysis};
use crate::model::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct StoreError(pub String);

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for StoreError {}

pub type StoreResult<T> = Result<T, StoreError>;

fn error(error: rusqlite::Error) -> StoreError {
    StoreError(error.to_string())
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct StoredEvent {
    pub seq: i64,
    pub branch: String,
    pub event: EventRecord,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct BranchState {
    pub name: String,
    pub parent_branch: Option<String>,
    pub parent_seq: Option<i64>,
    pub tip_seq: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReplayedState {
    pub input: Option<InputSnapshot>,
    pub overlay: DecisionOverlay,
}

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: &str) -> StoreResult<Self> {
        let connection = Connection::open(path).map_err(error)?;
        let store = Self { connection };
        store.init()?;
        Ok(store)
    }

    pub fn in_memory() -> StoreResult<Self> {
        let connection = Connection::open_in_memory().map_err(error)?;
        let store = Self { connection };
        store.init()?;
        Ok(store)
    }

    fn init(&self) -> StoreResult<()> {
        self.connection.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS events (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                branch TEXT NOT NULL,
                payload TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS branches (
                name TEXT PRIMARY KEY,
                parent_branch TEXT,
                parent_seq INTEGER,
                tip_seq INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS app_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT OR IGNORE INTO branches(name, parent_branch, parent_seq, tip_seq)
            VALUES ('main', NULL, NULL, 0);
            INSERT OR IGNORE INTO app_meta(key, value) VALUES ('branch', 'main');
            "#,
        ).map_err(error)
    }

    pub fn current_branch(&self) -> StoreResult<String> {
        self.connection
            .query_row("SELECT value FROM app_meta WHERE key = 'branch'", [], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(error)
            .map(|value| value.unwrap_or_else(|| "main".to_string()))
    }

    pub fn set_current_branch(&mut self, branch: &str) -> StoreResult<()> {
        let exists: bool = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM branches WHERE name = ?1)",
                params![branch],
                |row| row.get(0),
            )
            .map_err(error)?;
        if !exists {
            return Err(StoreError(format!("unknown branch {branch}")));
        }
        self.connection
            .execute(
                "INSERT INTO app_meta(key, value) VALUES('branch', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![branch],
            )
            .map_err(error)?;
        Ok(())
    }

    pub fn branches(&self) -> StoreResult<Vec<BranchState>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT name, parent_branch, parent_seq, tip_seq
                 FROM branches ORDER BY name",
            )
            .map_err(error)?;
        let rows = statement
            .query_map([], |row| {
                Ok(BranchState {
                    name: row.get(0)?,
                    parent_branch: row.get(1)?,
                    parent_seq: row.get(2)?,
                    tip_seq: row.get(3)?,
                })
            })
            .map_err(error)?;
        rows.collect::<Result<_, _>>().map_err(error)
    }

    fn append_on_branch(&mut self, branch: &str, event: &EventRecord) -> StoreResult<i64> {
        let payload = serde_json::to_string(event).map_err(|err| StoreError(err.to_string()))?;
        let tx = self.connection.transaction().map_err(error)?;
        let seq = tx
            .query_row(
                "INSERT INTO events(branch, payload) VALUES (?1, ?2) RETURNING seq",
                params![branch, payload],
                |row| row.get(0),
            )
            .map_err(error)?;
        tx.execute(
            "INSERT INTO branches(name, tip_seq) VALUES (?1, ?2)
             ON CONFLICT(name) DO UPDATE SET tip_seq = excluded.tip_seq",
            params![branch, seq],
        )
        .map_err(error)?;
        tx.commit().map_err(error)?;
        Ok(seq)
    }

    pub fn import_input(&mut self, input: InputSnapshot) -> StoreResult<i64> {
        if input.version.trim().is_empty() {
            return Err(StoreError("input version must not be empty".to_string()));
        }
        let branch = self.current_branch()?;
        let existing = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM events WHERE json_extract(payload, '$.type') = 'input_imported'
                 AND json_extract(payload, '$.0.version') = ?1)",
                params![input.version],
                |row| row.get::<_, bool>(0),
            )
            .optional()
            .map_err(error)?;
        if existing == Some(true) {
            return Err(StoreError(format!("input version {} already exists", input.version)));
        }
        self.append_on_branch(&branch, &EventRecord::InputImported(input))
    }

    pub fn create_branch(&mut self, name: &str, from_branch: &str, at_seq: Option<i64>) -> StoreResult<()> {
        if name.trim().is_empty() {
            return Err(StoreError("branch name must not be empty".to_string()));
        }
        let tip: i64 = self
            .connection
            .query_row(
                "SELECT tip_seq FROM branches WHERE name = ?1",
                params![from_branch],
                |row| row.get(0),
            )
            .optional()
            .map_err(error)?
            .ok_or_else(|| StoreError(format!("unknown branch {from_branch}")))?;
        let at = at_seq.unwrap_or(tip);
        if at < 0 || at > tip {
            return Err(StoreError("branch point is outside history".to_string()));
        }
        let tx = self.connection.transaction().map_err(error)?;
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM branches WHERE name = ?1)",
                params![name],
                |row| row.get(0),
            )
            .map_err(error)?;
        if exists {
            return Err(StoreError(format!("branch {name} already exists")));
        }
        tx.execute(
            "INSERT INTO branches(name, parent_branch, parent_seq, tip_seq)
             VALUES (?1, ?2, ?3, ?4)",
            params![name, from_branch, at, at],
        )
        .map_err(error)?;
        tx.execute(
            "INSERT INTO events(seq, branch, payload)
             SELECT seq, ?1, payload FROM events
             WHERE branch = ?2 AND seq <= ?3 ORDER BY seq",
            params![name, from_branch, at],
        )
        .map_err(error)?;
        tx.commit().map_err(error)?;
        Ok(())
    }

    pub fn rollback(&mut self, at_seq: i64) -> StoreResult<()> {
        let branch = self.current_branch()?;
        let row: Option<(Option<i64>, i64)> = self
            .connection
            .query_row(
                "SELECT parent_seq, tip_seq FROM branches WHERE name = ?1",
                params![branch],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(error)?;
        let (parent_seq, tip) = row.ok_or_else(|| StoreError("current branch missing".to_string()))?;
        if at_seq < parent_seq.unwrap_or(0) || at_seq > tip {
            return Err(StoreError("rollback point is outside this branch".to_string()));
        }
        let tx = self.connection.transaction().map_err(error)?;
        tx.execute(
            "DELETE FROM events WHERE branch = ?1 AND seq > ?2",
            params![branch, at_seq],
        )
        .map_err(error)?;
        tx.execute(
            "UPDATE branches SET tip_seq = ?1 WHERE name = ?2",
            params![at_seq, branch],
        )
        .map_err(error)?;
        tx.commit().map_err(error)?;
        Ok(())
    }

    fn history_events(&self, branch: &str) -> StoreResult<Vec<StoredEvent>> {
        let mut statement = self
            .connection
            .prepare("SELECT seq, payload FROM events WHERE branch = ?1 ORDER BY seq")
            .map_err(error)?;
        let rows = statement
            .query_map(params![branch], |row| {
                let seq: i64 = row.get(0)?;
                let payload: String = row.get(1)?;
                let event = serde_json::from_str(&payload).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?;
                Ok((seq, event))
            })
            .map_err(error)?;
        rows.map(|row| {
            row.map(|(seq, event)| StoredEvent {
                seq,
                branch: branch.to_string(),
                event,
            })
        })
        .collect::<Result<_, _>>()
        .map_err(error)
    }

    pub fn events(&self) -> StoreResult<Vec<StoredEvent>> {
        self.history_events(&self.current_branch()?)
    }

    pub fn replay(&self) -> StoreResult<ReplayedState> {
        let mut state = ReplayedState::default();
        for stored in self.events()? {
            match stored.event {
                EventRecord::InputImported(input) => {
                    state.input = Some(input);
                    state.overlay = DecisionOverlay::default();
                }
                EventRecord::Decision(decision) => {
                    if Some(decision.input_version()) != state.input.as_ref().map(|input| input.version.as_str()) {
                        continue;
                    }
                    match decision {
                        DecisionEvent::AcceptCandidate { block_id, candidate_id, note, .. } => {
                            state.overlay.accepted.push(AcceptedCandidate { block_id, candidate_id, note });
                        }
                        DecisionEvent::LockPhase { sample_id, chrom, start, end, assignment, note, .. } => {
                            state.overlay.locks.push(PhaseLock {
                                sample_id,
                                chrom,
                                start,
                                end,
                                assignment,
                                note,
                            });
                        }
                        DecisionEvent::WithdrawReadLink { link_id, reason, .. } => {
                            state.overlay.withdrawn_links.push((link_id, reason));
                        }
                        DecisionEvent::SetRelationshipStatus { relationship_id, status, .. } => {
                            state.overlay.relationship_status.insert(relationship_id, status);
                        }
                    }
                }
            }
        }
        Ok(state)
    }

    pub fn analysis(&self) -> StoreResult<(ReplayedState, Analysis)> {
        let state = self.replay()?;
        let analysis = match &state.input {
            Some(input) => analysis::analyze(input, &state.overlay, 8),
            None => Analysis {
                input_version: String::new(),
                blocks: Vec::new(),
                conflicts: Vec::new(),
                ignored_observations: Vec::new(),
                withdrawals: Vec::new(),
                relationship_status: BTreeMap::new(),
            },
        };
        Ok((state, analysis))
    }

    pub fn decide(&mut self, event: DecisionEvent) -> StoreResult<i64> {
        let state = self.replay()?;
        let input = state
            .input
            .clone()
            .ok_or_else(|| StoreError("no input snapshot imported".to_string()))?;
        if event.input_version() != input.version {
            return Err(StoreError(format!(
                "stale input version {}; current is {}",
                event.input_version(),
                input.version
            )));
        }
        let analysis = analysis::analyze(&input, &state.overlay, 8);
        analysis::validate_decision(&input, &analysis, &event).map_err(StoreError)?;
        let branch = self.current_branch()?;
        self.append_on_branch(&branch, &EventRecord::Decision(event))
    }

    pub fn export_bundle(&self) -> StoreResult<serde_json::Value> {
        let state = self.replay()?;
        let analysis = self.analysis()?.1;
        Ok(serde_json::json!({
            "format": "phase-weave-export-v1",
            "branch": self.current_branch()?,
            "pinned_input_version": state.input.as_ref().map(|input| input.version.clone()),
            "pinned_input": state.input,
            "events": self.events()?,
            "analysis": analysis,
        }))
    }

    pub fn latest_input_version(&self) -> StoreResult<Option<String>> {
        Ok(self.replay()?.input.map(|input| input.version))
    }
}
