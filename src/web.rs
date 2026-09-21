use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use rusqlite::params;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

use crate::db::{self, Db};
use crate::engine;
use crate::models::{
    BranchRequest, DecisionRequest, ImportPayload, PinRequest, RollbackRequest, SwitchRequest,
};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Db>>,
    pub candidate_budget: usize,
    pub default_branch: String,
}

pub fn app(db: Db, candidate_budget: usize, default_branch: String) -> Router {
    let state = AppState {
        db: Arc::new(Mutex::new(db)),
        candidate_budget,
        default_branch: default_branch.clone(),
    };
    Router::new()
        .route("/", get(index))
        .route("/api/state", get(state_handler).post(switch_branch))
        .route("/api/import", post(import_data))
        .route("/api/decisions", post(decision))
        .route("/api/branches", post(create_branch))
        .route("/api/pin", post(pin_version))
        .route("/api/rollback", post(rollback))
        .route("/api/export", get(export))
        .route("/api/replay", get(replay))
        .with_state(state)
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn state_handler(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let branch = current_branch(&state)?;
    let snapshot = with_db(&state, |conn| {
        engine::build_state(conn, &branch, state.candidate_budget).map_err(db_message)
    })
    .map_err(AppError::internal)?;
    Ok(Json(serde_json::to_value(snapshot).map_err(internal)?))
}

async fn import_data(
    State(state): State<AppState>,
    Json(payload): Json<ImportPayload>,
) -> Result<Json<Value>, AppError> {
    validate_import(&payload)?;
    let result = with_db(&state, |conn| {
        db::ensure_branches(conn)?;
        db::import_payload(conn, &payload)
    })
    .map_err(AppError::internal)?;
    Ok(Json(json!({ "version": result.0, "batch_id": result.1 })))
}

async fn decision(
    State(state): State<AppState>,
    Json(req): Json<DecisionRequest>,
) -> Result<Json<Value>, AppError> {
    validate_decision(&req)?;
    let branch = current_branch(&state)?;
    let payload = serde_json::to_value(&req).map_err(internal)?;
    let id = with_db(&state, |conn| {
        db::append_event(conn, &branch, &req.kind, &payload, req.input_version)
    })
    .map_err(AppError::internal)?;
    Ok(Json(json!({ "event_id": id, "branch": branch })))
}

async fn create_branch(
    State(state): State<AppState>,
    Json(req): Json<BranchRequest>,
) -> Result<Json<Value>, AppError> {
    if req.name.trim().is_empty() {
        return Err(client("分支名称不能为空"));
    }
    let result = with_db(&state, |conn| {
        db::ensure_branches(conn)?;
        let parent = if req.from_branch.trim().is_empty() { db::active_branch(conn)? } else { req.from_branch.clone() };
        let exists: i64 = conn.query_row("select count(*) from branches where name=?1", params![req.name], |r| r.get(0))?;
        if exists > 0 {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(DbMessage("branch exists".into()))));
        }
        let fork = conn.query_row("select max(event_id) from events where branch=?1", params![parent], |r| r.get::<_, Option<i64>>(0))?;
        conn.execute(
            "insert into branches(name, parent_branch, fork_event_id, created_at) values (?1,?2,?3,?4)",
            params![req.name, parent, fork, crate::db::now_ms_public()],
        )?;
        Ok::<(String, String, Option<i64>), rusqlite::Error>((req.name, parent, fork))
    }).map_err(|e| if e.to_string().contains("branch exists") { AppError::client("分支已存在") } else { AppError::internal(e) })?;
    Ok(Json(
        json!({ "branch": result.0, "parent_branch": result.1, "fork_event_id": result.2 }),
    ))
}

async fn switch_branch(
    State(state): State<AppState>,
    Json(req): Json<SwitchRequest>,
) -> Result<Json<Value>, AppError> {
    with_db(&state, |conn| {
        let exists: i64 = conn.query_row(
            "select count(*) from branches where name=?1",
            params![req.branch],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(db_message("missing branch"));
        }
        db::set_active_branch(conn, &req.branch)
    })
    .map_err(|e| {
        if e.to_string().contains("missing branch") {
            AppError::client("分支不存在")
        } else {
            AppError::internal(e)
        }
    })?;
    state_handler(State(state)).await
}

async fn pin_version(
    State(state): State<AppState>,
    Json(req): Json<PinRequest>,
) -> Result<Json<Value>, AppError> {
    let branch = current_branch(&state)?;
    let id = with_db(&state, |conn| {
        let exists: i64 = conn.query_row(
            "select count(*) from imports where version=?1",
            params![req.version],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(db_message("missing version"));
        }
        db::append_event(
            conn,
            &branch,
            "pin_version",
            &json!({ "version": req.version }),
            None,
        )
    })
    .map_err(|e| {
        if e.to_string().contains("missing version") {
            AppError::client("输入版本不存在")
        } else {
            AppError::internal(e)
        }
    })?;
    Ok(Json(
        json!({ "event_id": id, "pinned_version": req.version }),
    ))
}

async fn rollback(
    State(state): State<AppState>,
    Json(req): Json<RollbackRequest>,
) -> Result<Json<Value>, AppError> {
    let branch = current_branch(&state)?;
    let id = with_db(&state, |conn| {
        let kind: String = conn
            .query_row(
                "select kind from events where event_id=?1 and branch=?2",
                params![req.event_id, branch],
                |r| r.get(0),
            )
            .map_err(|_| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(DbMessage("missing event".into())))
            })?;
        if kind == "rollback_decision" || kind == "pin_version" {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                DbMessage("not rollbackable".into()),
            )));
        }
        db::append_event(
            conn,
            &branch,
            "rollback_decision",
            &json!({ "event_id": req.event_id }),
            None,
        )
    })
    .map_err(|e| {
        let message = e.to_string();
        if message.contains("missing event") {
            AppError::client("当前分支找不到该事件")
        } else if message.contains("not rollbackable") {
            AppError::client("该事件不能直接回退；请追加新的裁定事件")
        } else {
            AppError::internal(e)
        }
    })?;
    Ok(Json(
        json!({ "event_id": id, "rolled_back_event_id": req.event_id }),
    ))
}

async fn export(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let branch = current_branch(&state)?;
    let snapshot = with_db(&state, |conn| {
        engine::build_state(conn, &branch, state.candidate_budget)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(DbMessage(e))))
    })
    .map_err(AppError::internal)?;
    Ok(Json(engine::export_state(&snapshot)))
}

async fn replay(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let branch = current_branch(&state)?;
    let snapshot = with_db(&state, |conn| {
        engine::build_state(conn, &branch, state.candidate_budget)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(DbMessage(e))))
    })
    .map_err(AppError::internal)?;
    Ok(Json(json!({
        "branch": branch,
        "pinned_input_version": snapshot.pinned_version,
        "effective_input_version": snapshot.effective_version,
        "steps": snapshot.replay_steps,
    })))
}

fn current_branch(state: &AppState) -> Result<String, AppError> {
    with_db(state, |conn| {
        db::ensure_branches(conn)?;
        let exists: i64 = conn.query_row(
            "select count(*) from app_state where key='active_branch'",
            [],
            |r| r.get(0),
        )?;
        if exists == 0 {
            Ok(state.default_branch.clone())
        } else {
            db::active_branch(conn)
        }
    })
    .map_err(AppError::internal)
}

fn with_db<T>(
    state: &AppState,
    f: impl FnOnce(&rusqlite::Connection) -> rusqlite::Result<T>,
) -> rusqlite::Result<T> {
    let guard = state.db.lock().map_err(|_| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(DbMessage(
            "database lock poisoned".into(),
        )))
    })?;
    f(&guard.0)
}

fn validate_import(payload: &ImportPayload) -> Result<(), AppError> {
    if payload.samples.is_empty() {
        return Err(client("至少需要一个样本"));
    }
    if payload.variants.is_empty() {
        return Err(client("至少需要一个变异位置"));
    }
    Ok(())
}

fn validate_decision(req: &DecisionRequest) -> Result<(), AppError> {
    match req.kind.as_str() {
        "accept_candidate" if req.block_id.is_empty() || req.candidate_signature.is_empty() => {
            Err(client("接受候选需要 block_id 和 candidate_signature"))
        }
        "lock_phase"
            if req.sample_id.is_empty()
                || req.variant_id.is_empty()
                || !["father", "mother"].contains(&req.haplotype_a_origin.as_str()) =>
        {
            Err(client(
                "锁定需要 sample_id、variant_id 以及 haplotype_a_origin=father|mother",
            ))
        }
        "withdraw_read_link" if req.read_link_id.is_empty() => Err(client("撤回需要 read_link_id")),
        "revise_relationship" | "mark_relationship_uncertain" if req.relationship_id.is_empty() => {
            Err(client("关系修订需要 relationship_id"))
        }
        "accept_de_novo" if req.conflict_id.is_empty() => {
            Err(client("接受 de novo 需要 conflict_id"))
        }
        kind if [
            "accept_candidate",
            "lock_phase",
            "withdraw_read_link",
            "revise_relationship",
            "mark_relationship_uncertain",
            "accept_de_novo",
        ]
        .contains(&kind) =>
        {
            Ok(())
        }
        _ => Err(client("未知裁定类型")),
    }
}

#[derive(Debug)]
struct DbMessage(String);
impl std::fmt::Display for DbMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for DbMessage {}

fn db_message(message: impl Into<String>) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(DbMessage(message.into())))
}

#[derive(Debug)]
struct AppError {
    status: StatusCode,
    message: String,
}
impl AppError {
    fn client(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
    fn internal(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}
fn client(message: impl Into<String>) -> AppError {
    AppError::client(message)
}
fn internal(error: impl std::fmt::Display) -> AppError {
    AppError::internal(error)
}
impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}
