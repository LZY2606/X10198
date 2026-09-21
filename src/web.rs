use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::domain::{Analysis, Branch, Decision, DecisionEvent};
use crate::engine;
use crate::repository::{Repository, StoreError};

#[derive(Clone)]
pub struct AppState {
    pub repository: Arc<Repository>,
    pub budget: usize,
}

#[derive(Debug, Deserialize)]
pub struct ViewQuery {
    pub version: Option<String>,
    pub branch: Option<String>,
    pub budget: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct AppendRequest {
    pub branch: Option<String>,
    pub version: Option<String>,
    pub parent_event_id: Option<String>,
    pub decision: Decision,
}

#[derive(Debug, Deserialize)]
pub struct RollbackRequest {
    pub branch: Option<String>,
    pub target_event_id: Option<String>,
    pub label: String,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Debug, Serialize)]
struct StateResponse {
    versions: Vec<VersionResponse>,
    branches: Vec<Branch>,
    selected_version: String,
    selected_branch: String,
    analysis: Analysis,
}

#[derive(Debug, Serialize)]
struct VersionResponse {
    id: String,
    label: String,
    checksum: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/state", get(view_state))
        .route("/api/decisions", post(append_decision))
        .route("/api/rollback", post(rollback))
        .route("/api/export", get(export))
        .with_state(state)
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn view_state(
    State(app_state): State<AppState>,
    Query(query): Query<ViewQuery>,
) -> Result<Json<StateResponse>, ApiError> {
    let selected_version = match query.version {
        Some(version) => version,
        None => app_state.repository.latest_version_id()?,
    };
    let selected_branch = query.branch.unwrap_or_else(|| "main".to_string());
    let snapshot = app_state.repository.snapshot(&selected_version)?;
    let all_events = app_state.repository.events_for_branch(&selected_branch)?;
    let effective_ids: std::collections::BTreeSet<String> = all_events
        .iter()
        .filter(|event| event.pinned_version == selected_version)
        .map(|event| event.id.clone())
        .collect();
    let effective: Vec<DecisionEvent> = all_events
        .iter()
        .filter(|event| effective_ids.contains(&event.id))
        .cloned()
        .collect();
    let stale: Vec<DecisionEvent> = all_events
        .into_iter()
        .filter(|event| event.pinned_version != selected_version)
        .collect();
    let analysis = engine::analyze(
        &snapshot,
        &selected_branch,
        &effective,
        &stale,
        query.budget.unwrap_or(app_state.budget),
    );
    let versions = app_state
        .repository
        .versions()?
        .into_iter()
        .map(|(id, label, checksum)| VersionResponse {
            id,
            label,
            checksum,
        })
        .collect();
    Ok(Json(StateResponse {
        versions,
        branches: app_state.repository.branches()?,
        selected_version,
        selected_branch,
        analysis,
    }))
}

async fn append_decision(
    State(app_state): State<AppState>,
    Json(request): Json<AppendRequest>,
) -> Result<Json<Analysis>, ApiError> {
    let branch = request.branch.unwrap_or_else(|| "main".to_string());
    let version = match request.version {
        Some(version) => version,
        None => app_state.repository.latest_version_id()?,
    };
    let _event = app_state
        .repository
        .append_decision(&branch, &version, request.parent_event_id, request.decision)?;
    let analysis = current_analysis(&app_state, branch, version, app_state.budget)?;
    Ok(Json(analysis))
}

async fn rollback(
    State(app_state): State<AppState>,
    Json(request): Json<RollbackRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let branch = request.branch.unwrap_or_else(|| "main".to_string());
    let (_new_branch, marker) = app_state
        .repository
        .rollback_to_event(&branch, request.target_event_id, &request.label)?;
    Ok(Json(serde_json::json!({
        "branch_id": marker.branch_id,
        "marker_event": marker
    })))
}

async fn export(
    State(app_state): State<AppState>,
    Query(query): Query<ViewQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let version = query.version.unwrap_or(app_state.repository.latest_version_id()?);
    let branch = query.branch.unwrap_or_else(|| "main".to_string());
    Ok(Json(app_state.repository.export(&branch, &version)?))
}

fn current_analysis(
    app_state: &AppState,
    branch: String,
    version: String,
    budget: usize,
) -> Result<Analysis, StoreError> {
    let snapshot = app_state.repository.snapshot(&version)?;
    let events = app_state.repository.events_for_branch(&branch)?;
    let effective = events
        .iter()
        .filter(|event| event.pinned_version == version)
        .cloned()
        .collect::<Vec<_>>();
    let stale: Vec<crate::domain::DecisionEvent> = events
        .into_iter()
        .filter(|event| event.pinned_version != version)
        .collect();
    Ok(engine::analyze(
        &snapshot,
        &branch,
        &effective,
        &stale,
        budget,
    ))
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

impl From<StoreError> for ApiError {
    fn from(value: StoreError) -> Self {
        let status = match value {
            StoreError::Conflict(_) => StatusCode::CONFLICT,
            StoreError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError {
            status,
            message: value.to_string(),
        }
    }
}
