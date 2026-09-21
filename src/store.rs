use crate::domain::{EventRecord, ImportPayload};
use crate::engine::State;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Debug)]
pub struct AppError(pub String);

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
}

impl std::error::Error for AppError {}

pub type AppResult<T> = Result<T, AppError>;

fn fail(message: impl Into<String>) -> AppError { AppError(message.into()) }

#[derive(Clone)]
pub struct Store {
    connection: std::sync::Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone)]
pub struct ViewState {
    pub state: State,
    pub events: Vec<EventRecord>,
    pub active_branch: String,
}

impl Store {
    pub fn open(path: &str) -> AppResult<Self> {
        let connection = Connection::open(path).map_err(|e| fail(e.to_string()))?;
        connection.pragma_update(None, "foreign_keys", "on").ok();
        connection.execute_batch(SCHEMA).map_err(|e| fail(e.to_string()))?;
        let store = Self { connection: std::sync::Arc::new(Mutex::new(connection)) };
        store.bootstrap();
        Ok(store)
    }

    fn bootstrap(&self) {
        let conn = self.connection.lock().unwrap();
        conn.execute("insert or ignore into meta(key, value) values ('active_branch', 'main')", []).ok();
        conn.execute("insert or ignore into branches(name, parent_event_id, status) values ('main', null, 'active')", []).ok();
    }

    pub fn import(&self, payload: ImportPayload) -> AppResult<EventRecord> {
        validate_import(&payload)?;
        let mut conn = self.connection.lock().unwrap();
        let tx = conn.transaction().map_err(|e| fail(e.to_string()))?;
        let already = tx.query_row("select 1 from import_versions where version = ?1", params![payload.version], |_| Ok(1)).optional().map_err(|e| fail(e.to_string()))?;
        if already.is_some() { return Err(fail(format!("import version {} already exists", payload.version))); }
        let json = serde_json::to_string(&payload).map_err(|e| fail(e.to_string()))?;
        let branch = active_branch_locked(&tx)?;
        let sequence = next_sequence_locked(&tx, &branch)?;
        let pinned = current_pinned_locked(&tx, &branch)?;
        tx.execute("insert into import_versions(version, payload_json, payload_hash, received_at) values (?1, ?2, ?3, datetime('now'))",
            params![payload.version, json, hash_text(&json)]).map_err(|e| fail(e.to_string()))?;
        let first_version = tx.query_row("select count(*) from import_versions", [], |row| row.get::<_, i64>(0)).unwrap_or(1) == 1;
        tx.execute("insert into event_log(branch, sequence, kind, payload_json, parent_id, pinned_version) values (?1, ?2, 'import', ?3, ?4, ?5)",
            params![branch, sequence, json, pinned, pinned]).map_err(|e| fail(e.to_string()))?;
        if first_version {
            let id = tx.last_insert_rowid();
            let sequence = next_sequence_locked(&tx, &branch)?;
            tx.execute("insert into event_log(branch, sequence, kind, payload_json, parent_id, pinned_version) values (?1, ?2, 'pin_version', json_object('version', ?3), ?4, ?3)",
                params![branch, sequence, payload.version, id, payload.version]).map_err(|e| fail(e.to_string()))?;
        }
        tx.commit().map_err(|e| fail(e.to_string()))?;
        self.latest_event()
    }

    pub fn decide(&self, kind: &str, payload: serde_json::Value) -> AppResult<EventRecord> {
        validate_decision(kind, &payload)?;
        let mut conn = self.connection.lock().unwrap();
        let tx = conn.transaction().map_err(|e| fail(e.to_string()))?;
        let branch = active_branch_locked(&tx)?;
        let state = reconstruct_locked(&tx, &branch)?;
        validate_against_state(kind, &payload, &state)?;
        let sequence = next_sequence_locked(&tx, &branch)?;
        let parent = last_event_id_locked(&tx, &branch)?;
        let pinned = state.state.pinned_version.clone().or(state.state.current_version.clone());
        let json = serde_json::to_string(&payload).map_err(|e| fail(e.to_string()))?;
        tx.execute("insert into event_log(branch, sequence, kind, payload_json, parent_id, pinned_version) values (?1, ?2, ?3, ?4, ?5, ?6)",
            params![branch, sequence, kind, json, parent, pinned]).map_err(|e| fail(e.to_string()))?;
        tx.commit().map_err(|e| fail(e.to_string()))?;
        self.latest_event()
    }

    pub fn rollback(&self, event_id: i64, new_branch: String) -> AppResult<String> {
        let mut conn = self.connection.lock().unwrap();
        let tx = conn.transaction().map_err(|e| fail(e.to_string()))?;
        let existing: Option<String> = tx.query_row("select name from branches where name = ?1", params![new_branch], |row| row.get(0)).optional().map_err(|e| fail(e.to_string()))?;
        if existing.is_some() { return Err(fail(format!("branch {new_branch} already exists"))); }
        let target = event_record_locked(&tx, event_id)?;
        tx.execute("insert into branches(name, parent_event_id, status) values (?1, ?2, 'active')",
            params![new_branch, target.id]).map_err(|e| fail(e.to_string()))?;
        tx.execute("update meta set value = ?1 where key = 'active_branch'", params![new_branch]).map_err(|e| fail(e.to_string()))?;
        tx.commit().map_err(|e| fail(e.to_string()))?;
        Ok(new_branch)
    }

    pub fn latest_event(&self) -> AppResult<EventRecord> {
        let conn = self.connection.lock().unwrap();
        let branch = active_branch_locked(&conn)?;
        let id = conn.last_insert_rowid();
        if id > 0 {
            if let Ok(event) = event_record_locked(&conn, id) { return Ok(event); }
        }
        conn.query_row("select * from event_log where branch = ?1 order by sequence desc limit 1", params![branch], map_event)
            .map_err(|e| fail(e.to_string()))
    }

    pub fn view(&self) -> AppResult<ViewState> {
        let conn = self.connection.lock().unwrap();
        let branch = active_branch_locked(&conn)?;
        Ok(reconstruct_locked(&conn, &branch)?)
    }

    pub fn events(&self) -> AppResult<Vec<EventRecord>> {
        let conn = self.connection.lock().unwrap();
        let branch = active_branch_locked(&conn)?;
        let mut stmt = conn.prepare("select * from event_log where branch = ?1 order by sequence").map_err(|e| fail(e.to_string()))?;
        let rows = stmt.query_map(params![branch], map_event).map_err(|e| fail(e.to_string()))?;
        rows.collect::<Result<_, _>>().map_err(|e| fail(e.to_string()))
    }

    pub fn branches(&self) -> AppResult<Vec<(String, Option<i64>, String)>> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare("select name, parent_event_id, status from branches order by created_at, name").map_err(|e| fail(e.to_string()))?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).map_err(|e| fail(e.to_string()))?;
        rows.collect::<Result<_, _>>().map_err(|e| fail(e.to_string()))
    }

    pub fn export(&self) -> AppResult<serde_json::Value> {
        let view = self.view()?;
        let events = self.events()?;
        let inputs = self.import_payloads()?;
        Ok(serde_json::json!({
            "application": "相位织图",
            "offline": true,
            "active_branch": view.active_branch,
            "pinned_input_version": view.state.pinned_version,
            "current_input_version": view.state.current_version,
            "pinned_inputs": inputs,
            "adjudication_events": events.iter().filter(|event| event.kind != "import").collect::<Vec<_>>(),
            "replay_steps": events.iter().map(step_event).collect::<Vec<_>>(),
        }))
    }

    fn import_payloads(&self) -> AppResult<Vec<ImportPayload>> {
        let view = self.view()?;
        let version = view.state.pinned_version.or(view.state.current_version).ok_or_else(|| fail("no pinned input version"))?;
        let conn = self.connection.lock().unwrap();
        let json: String = conn.query_row("select payload_json from import_versions where version = ?1", params![version], |row| row.get(0)).map_err(|e| fail(e.to_string()))?;
        Ok(vec![serde_json::from_str(&json).map_err(|e| fail(e.to_string()))?])
    }

    pub fn replay(&self) -> AppResult<Vec<serde_json::Value>> {
        Ok(self.events()?.iter().map(step_event).collect())
    }
}

fn reconstruct_locked(conn: &Connection, branch: &str) -> AppResult<ViewState> {
    let mut state = State::new();
    let mut events = Vec::new();
    let mut stmt = conn.prepare("select * from event_log where branch = ?1 order by sequence").map_err(|e| fail(e.to_string()))?;
    let rows = stmt.query_map(params![branch], map_event).map_err(|e| fail(e.to_string()))?;
    for row in rows {
        let event = row.map_err(|e| fail(e.to_string()))?;
        state.apply(&event.kind, event.payload.clone());
        events.push(event);
    }
    Ok(ViewState { state, events, active_branch: branch.to_string() })
}

fn active_branch_locked(conn: &Connection) -> AppResult<String> {
    conn.query_row("select value from meta where key='active_branch'", [], |row| row.get(0)).map_err(|e| fail(e.to_string()))
}

fn next_sequence_locked(conn: &Connection, branch: &str) -> AppResult<i64> {
    conn.query_row("select coalesce(max(sequence), 0) + 1 from event_log where branch = ?1", params![branch], |row| row.get(0)).map_err(|e| fail(e.to_string()))
}

fn last_event_id_locked(conn: &Connection, branch: &str) -> AppResult<Option<i64>> {
    conn.query_row("select max(id) from event_log where branch = ?1", params![branch], |row| row.get(0)).map_err(|e| fail(e.to_string()))
}

fn event_record_locked(conn: &Connection, id: i64) -> AppResult<EventRecord> {
    conn.query_row("select * from event_log where id = ?1", params![id], map_event).map_err(|e| fail(e.to_string()))
}

fn map_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventRecord> {
    let json: String = row.get(4)?;
    Ok(EventRecord {
        id: row.get(0)?,
        branch: row.get(1)?,
        sequence: row.get(2)?,
        kind: row.get(3)?,
        payload: serde_json::from_str(&json).unwrap_or(serde_json::Value::Null),
        parent_id: row.get(5)?,
        pinned_version: row.get(6)?,
        created_at: row.get(7)?,
        applied: true,
    })
}

fn step_event(event: &EventRecord) -> serde_json::Value {
    serde_json::json!({"sequence": event.sequence, "id": event.id, "kind": event.kind, "parent_id": event.parent_id, "pinned_version": event.pinned_version, "payload": event.payload})
}

fn hash_text(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() { hash ^= byte as u64; hash = hash.wrapping_mul(0x100000001b3); }
    format!("{hash:016x}")
}
