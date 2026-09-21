use crate::model::*;
use crate::web::index_html;
use crate::{App, AppError};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/state", get(state))
        .route("/api/import", post(import_bundle))
        .route("/api/decide", post(decide))
        .route("/api/rollback", post(rollback))
        .route("/api/fork", post(fork))
        .route("/api/export", get(export))
        .route("/api/replay", post(replay))
        .route("/api/links/{id}/retract", post(retract_link))
        .with_state(app)
}

async fn index() -> Html<String> {
    Html(index_html())
}

#[derive(Deserialize)]
struct BranchQuery {
    #[serde(default = "default_branch")]
    branch: String,
}

fn default_branch() -> String {
    "main".to_string()
}

async fn state(
    State(app): State<Arc<App>>,
    Query(q): Query<BranchQuery>,
) -> Json<serde_json::Value> {
    Json(serde_json::to_value(app.view(&q.branch)).unwrap())
}

async fn import_bundle(
    State(app): State<Arc<App>>,
    Json(bundle): Json<ImportBundle>,
) -> Json<serde_json::Value> {
    let version = app.import(&bundle);
    Json(serde_json::json!({ "input_version": version }))
}

#[derive(Deserialize)]
struct DecideReq {
    #[serde(default = "default_branch")]
    branch: String,
    expected_version: i64,
    decision: Decision,
}

async fn decide(
    State(app): State<Arc<App>>,
    Json(req): Json<DecideReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let seq = app.decide(&req.branch, req.expected_version, req.decision)?;
    Ok(Json(serde_json::json!({ "seq": seq })))
}

#[derive(Deserialize)]
struct BranchReq {
    #[serde(default = "default_branch")]
    branch: String,
}

async fn rollback(
    State(app): State<Arc<App>>,
    Json(req): Json<BranchReq>,
) -> Json<serde_json::Value> {
    match app.rollback(&req.branch) {
        Some(ev) => Json(serde_json::json!({ "rolled_back": ev })),
        None => Json(serde_json::json!({ "rolled_back": null })),
    }
}

#[derive(Deserialize)]
struct ForkReq {
    from: String,
    to: String,
    at_seq: Option<i64>,
}

async fn fork(
    State(app): State<Arc<App>>,
    Json(req): Json<ForkReq>,
) -> Json<serde_json::Value> {
    app.fork(&req.from, &req.to, req.at_seq);
    Json(serde_json::json!({ "branches": app.branches() }))
}

async fn export(
    State(app): State<Arc<App>>,
    Query(q): Query<BranchQuery>,
) -> Json<ExportBundle> {
    Json(app.export(&q.branch))
}

#[derive(Deserialize)]
struct ReplayReq {
    snapshot: ExportBundle,
    raw: ImportBundle,
}

async fn replay(
    State(app): State<Arc<App>>,
    Json(req): Json<ReplayReq>,
) -> Json<serde_json::Value> {
    App::replay(&app, &req.snapshot, &req.raw);
    Json(serde_json::json!({ "replayed": true, "input_version": req.snapshot.input_version }))
}

#[derive(Deserialize)]
struct RetractReq {
    #[serde(default = "default_branch")]
    branch: String,
    expected_version: i64,
}

async fn retract_link(
    State(app): State<Arc<App>>,
    Path(id): Path<i64>,
    Json(req): Json<RetractReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let seq =
        app.decide(&req.branch, req.expected_version, Decision::RetractReadLink { link_id: id })?;
    Ok(Json(serde_json::json!({ "seq": seq })))
}

struct ApiError(AppError);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0 {
            AppError::VersionMismatch { .. } => StatusCode::CONFLICT,
            AppError::Invalid(_) => StatusCode::BAD_REQUEST,
            AppError::Db(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(serde_json::json!({ "error": self.0.to_string() }))).into_response()
    }
}

impl From<AppError> for ApiError {
    fn from(e: AppError) -> Self {
        ApiError(e)
    }
}
