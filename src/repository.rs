use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::{Branch, DecisionEvent, InputSnapshot};
use crate::seed;

#[derive(Debug)]
pub enum StoreError {
    Sql(rusqlite::Error),
    Json(serde_json::Error),
    Conflict(String),
    NotFound(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Sql(error) => write!(formatter, "SQLite error: {error}"),
            StoreError::Json(error) => write!(formatter, "JSON error: {error}"),
            StoreError::Conflict(message) => write!(formatter, "{message}"),
            StoreError::NotFound(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(value: rusqlite::Error) -> Self {
        StoreError::Sql(value)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(value: serde_json::Error) -> Self {
        StoreError::Json(value)
    }
}

pub struct Repository {
    connection: Mutex<Connection>,
}

impl Repository {
    pub fn open(path: &str) -> Result<Self, StoreError> {
        let connection = Connection::open(path)?;
        let repository = Repository {
            connection: Mutex::new(connection),
        };
        repository.migrate()?;
        repository.seed_if_empty()?;
        Ok(repository)
    }

    pub fn in_memory() -> Result<Self, StoreError> {
        let connection = Connection::open_in_memory()?;
        let repository = Repository {
            connection: Mutex::new(connection),
        };
        repository.migrate()?;
        repository.seed_if_empty()?;
        Ok(repository)
    }

    fn migrate(&self) -> Result<(), StoreError> {
        let connection = self.connection.lock().expect("repository mutex");
        connection.execute_batch(
            r#"
            create table if not exists schema_meta (
                key text primary key,
                value text not null
            );
            create table if not exists input_versions (
                id text primary key,
                label text not null,
                checksum text not null,
                snapshot_json text not null,
                created_seq integer not null
            );
            create table if not exists branches (
                id text primary key,
                parent_branch_id text references branches(id),
                created_by_event_id text,
                label text not null,
                head_event_id text
            );
            create table if not exists events (
                id text primary key,
                seq integer not null unique,
                branch_id text not null references branches(id),
                parent_event_id text,
                pinned_version text not null references input_versions(id),
                decision_json text not null
            );
            "#,
        )?;
        Ok(())
    }

    fn seed_if_empty(&self) -> Result<(), StoreError> {
        let mut connection = self.connection.lock().expect("repository mutex");
        let count: i64 =
            connection.query_row("select count(*) from input_versions", [], |row| row.get(0))?;
        if count > 0 {
            return Ok(());
        }
        let transaction = connection.transaction()?;
        for (created_seq, snapshot) in seed::all_versions().into_iter().enumerate() {
            let json = serde_json::to_string(&snapshot)?;
            transaction.execute(
                "insert into input_versions (id, label, checksum, snapshot_json, created_seq)
                 values (?1, ?2, ?3, ?4, ?5)",
                params![
                    snapshot.version.id,
                    snapshot.version.label,
                    snapshot.version.checksum,
                    json,
                    created_seq as i64
                ],
            )?;
        }
        transaction.execute(
            "insert into branches (id, parent_branch_id, created_by_event_id, label, head_event_id)
             values ('main', null, null, '主裁定分支', null)",
            [],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn latest_version_id(&self) -> Result<String, StoreError> {
        let connection = self.connection.lock().expect("repository mutex");
        connection
            .query_row(
                "select id from input_versions order by created_seq desc limit 1",
                [],
                |row| row.get(0),
            )
            .map_err(StoreError::Sql)
    }

    pub fn snapshot(&self, version_id: &str) -> Result<InputSnapshot, StoreError> {
        let connection = self.connection.lock().expect("repository mutex");
        let json: String = connection
            .query_row(
                "select snapshot_json from input_versions where id = ?1",
                params![version_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| StoreError::NotFound(format!("未知输入版本 {version_id}")))?;
        Ok(serde_json::from_str(&json)?)
    }

    pub fn versions(&self) -> Result<Vec<(String, String, String)>, StoreError> {
        let connection = self.connection.lock().expect("repository mutex");
        let mut statement =
            connection.prepare("select id, label, checksum from input_versions order by created_seq")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.collect::<Result<_, _>>().map_err(StoreError::Sql)
    }

    pub fn branches(&self) -> Result<Vec<Branch>, StoreError> {
        let connection = self.connection.lock().expect("repository mutex");
        let mut statement = connection.prepare(
            "select id, parent_branch_id, created_by_event_id, label, head_event_id
             from branches order by id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(Branch {
                id: row.get(0)?,
                parent_branch_id: row.get(1)?,
                created_by_event_id: row.get(2)?,
                label: row.get(3)?,
                head_event_id: row.get(4)?,
            })
        })?;
        rows.collect::<Result<_, _>>().map_err(StoreError::Sql)
    }

    pub fn events_for_branch(&self, branch_id: &str) -> Result<Vec<DecisionEvent>, StoreError> {
        let mut connection = self.connection.lock().expect("repository mutex");
        let transaction = connection.transaction()?;
        let branch_exists: bool = transaction.query_row(
            "select exists(select 1 from branches where id = ?1)",
            params![branch_id],
            |row| row.get(0),
        )?;
        if !branch_exists {
            return Err(StoreError::NotFound(format!("未知分支 {branch_id}")));
        }

        let mut chain = Vec::new();
        let mut current = Some(branch_id.to_string());
        while let Some(branch) = current {
            let parent_branch_id: Option<String> = transaction.query_row(
                "select parent_branch_id from branches where id = ?1",
                params![branch],
                |row| row.get(0),
            )?;
            let mut statement = transaction.prepare(
                "select id, seq, branch_id, parent_event_id, pinned_version, decision_json
                 from events where branch_id = ?1 order by seq",
            )?;
            let rows = statement.query_map(params![branch], map_event)?;
            let mut events = rows.collect::<Result<Vec<_>, _>>()?;
            chain.append(&mut events);
            current = parent_branch_id;
        }
        chain.sort_by_key(|event| event.seq);
        Ok(chain)
    }

    pub fn append_decision(
        &self,
        branch_id: &str,
        pinned_version: &str,
        parent_event_id: Option<String>,
        decision: crate::domain::Decision,
    ) -> Result<DecisionEvent, StoreError> {
        let mut connection = self.connection.lock().expect("repository mutex");
        let transaction = connection.transaction()?;
        let version_exists: bool = transaction.query_row(
            "select exists(select 1 from input_versions where id = ?1)",
            params![pinned_version],
            |row| row.get(0),
        )?;
        if !version_exists {
            return Err(StoreError::Conflict(format!(
                "不能钉住未知输入版本 {pinned_version}"
            )));
        }
        let branch_head: Option<String> = transaction
            .query_row(
                "select head_event_id from branches where id = ?1",
                params![branch_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| StoreError::NotFound(format!("未知分支 {branch_id}")))?;
        if parent_event_id != branch_head {
            return Err(StoreError::Conflict(
                "并发裁定冲突：parent_event_id 不是当前分支头，请刷新后基于最新裁定提交".to_string(),
            ));
        }
        let next_seq: i64 = transaction.query_row(
            "select coalesce(max(seq), 0) + 1 from events",
            [],
            |row| row.get(0),
        )?;
        let event_id = format!("evt-{next_seq:04}");
        let decision_json = serde_json::to_string(&decision)?;
        transaction.execute(
            "insert into events
             (id, seq, branch_id, parent_event_id, pinned_version, decision_json)
             values (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event_id,
                next_seq,
                branch_id,
                parent_event_id,
                pinned_version,
                decision_json
            ],
        )?;
        transaction.execute(
            "update branches set head_event_id = ?1 where id = ?2",
            params![event_id, branch_id],
        )?;
        transaction.commit()?;
        Ok(DecisionEvent {
            id: event_id,
            seq: next_seq,
            branch_id: branch_id.to_string(),
            parent_event_id,
            pinned_version: pinned_version.to_string(),
            decision,
        })
    }

    pub fn rollback_to_event(
        &self,
        branch_id: &str,
        target_event_id: Option<String>,
        label: &str,
    ) -> Result<(Branch, DecisionEvent), StoreError> {
        let events = self.events_for_branch(branch_id)?;
        let target_index = match &target_event_id {
            Some(event_id) => events
                .iter()
                .position(|event| event.id == *event_id)
                .ok_or_else(|| StoreError::NotFound(format!("分支上找不到事件 {event_id}")))?,
            None => usize::MAX,
        };
        let retained: Vec<DecisionEvent> = if target_event_id.is_none() {
            Vec::new()
        } else {
            events[..=target_index].to_vec()
        };
        let new_branch_id = format!("rollback-{}-{}", branch_id, events.len() + 1);
        let rollback_seq: i64 = {
            let connection = self.connection.lock().expect("repository mutex");
            connection.query_row("select coalesce(max(seq), 0) + 1 from events", [], |row| {
                row.get(0)
            })?
        };
        let rollback_event_id = format!("evt-{rollback_seq:04}");
        let mut connection = self.connection.lock().expect("repository mutex");
        let transaction = connection.transaction()?;
        transaction.execute(
            "insert into branches
             (id, parent_branch_id, created_by_event_id, label, head_event_id)
             values (?1, ?2, ?3, ?4, ?5)",
                params![
                    new_branch_id,
                    branch_id,
                    rollback_event_id,
                    label,
                    target_event_id.clone()
                ],
        )?;
        let pinned_version = retained
            .last()
            .map(|event| event.pinned_version.clone())
            .unwrap_or_else(|| "v1".to_string());
        let rollback_decision = crate::domain::Decision::RollbackMarker {
            target_event_id: target_event_id.clone(),
            label: label.to_string(),
        };
        let rollback_json = serde_json::to_string(&rollback_decision)?;
        transaction.execute(
            "insert into events
             (id, seq, branch_id, parent_event_id, pinned_version, decision_json)
             values (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                rollback_event_id,
                rollback_seq,
                new_branch_id,
                target_event_id.clone(),
                pinned_version,
                rollback_json
            ],
        )?;
        transaction.commit()?;

        let branch = Branch {
            id: new_branch_id,
            parent_branch_id: Some(branch_id.to_string()),
            created_by_event_id: Some(rollback_event_id.clone()),
            label: label.to_string(),
            head_event_id: target_event_id.clone(),
        };
        let event = DecisionEvent {
            id: rollback_event_id,
            seq: rollback_seq,
            branch_id: branch.id.clone(),
            parent_event_id: target_event_id,
            pinned_version,
            decision: rollback_decision,
        };
        Ok((branch, event))
    }

    pub fn export(&self, branch_id: &str, version_id: &str) -> Result<serde_json::Value, StoreError> {
        let snapshot = self.snapshot(version_id)?;
        let events = self.events_for_branch(branch_id)?;
        let branches = self.branches()?;
        Ok(serde_json::json!({
            "pinned_input_version": snapshot.version,
            "branch_id": branch_id,
            "input_snapshot": snapshot,
            "replay_events": events,
            "branches": branches,
            "offline_only": true,
            "external_reference_queries": 0
        }))
    }
}

fn map_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<DecisionEvent> {
    let decision_json: String = row.get(5)?;
    let decision: crate::domain::Decision = serde_json::from_str(&decision_json).map_err(
        |error| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        },
    )?;
    Ok(DecisionEvent {
        id: row.get(0)?,
        seq: row.get(1)?,
        branch_id: row.get(2)?,
        parent_event_id: row.get(3)?,
        pinned_version: row.get(4)?,
        decision,
    })
}
