use std::sync::Arc;
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use crate::model::Decision;
use crate::store::Store;

#[derive(Clone)]
pub struct AppState { pub store: Arc<Store> }

pub fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/state", get(get_state))
        .route("/api/branches", get(branches))
        .route("/api/events", get(events))
        .route("/api/export", get(export))
        .route("/api/decisions", post(decide))
        .route("/api/rollback", post(rollback))
        .route("/api/fork", post(fork))
        .route("/api/branches/{id}/select", post(select_branch))
        .with_state(AppState { store })
}

async fn index() -> Html<&'static str> { Html(include_str!("../static/index.html")) }

async fn get_state(State(state): State<AppState>) -> Response {
    match state.store.current_analysis() {
        Ok((branch, analysis, events)) => Json(serde_json::json!({ "branch": branch, "analysis": analysis, "events": events })).into_response(),
        Err(error) => error_response(error),
    }
}
async fn branches(State(state): State<AppState>) -> Response {
    match state.store.branches() { Ok(branches) => Json(branches).into_response(), Err(error) => error_response(error) }
}
async fn events(State(state): State<AppState>) -> Response {
    match state.store.all_events() { Ok(events) => Json(events).into_response(), Err(error) => error_response(error) }
}
async fn export(State(state): State<AppState>) -> Response {
    match state.store.export_current() {
        Ok(bundle) => (StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8"), (header::CONTENT_DISPOSITION, "attachment; filename=\"phase-weave-export.json\"")],
            Json(bundle)).into_response(),
        Err(error) => error_response(error),
    }
}
#[derive(Debug, Deserialize)]
struct ForkRequest { name: String }
async fn decide(State(state): State<AppState>, Json(decision): Json<Decision>) -> Response {
    match state.store.record_decision(decision) { Ok(event) => Json(event).into_response(), Err(error) => error_response(error) }
}
async fn rollback(State(state): State<AppState>) -> Response {
    match state.store.rollback_current() { Ok(branch) => Json(branch).into_response(), Err(error) => error_response(error) }
}
async fn fork(State(state): State<AppState>, Json(request): Json<ForkRequest>) -> Response {
    match state.store.fork_current(&request.name) { Ok(branch) => Json(branch).into_response(), Err(error) => error_response(error) }
}
async fn select_branch(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match state.store.set_current_branch(&id) {
        Ok(()) => get_state(State(state)).await,
        Err(error) => error_response(error),
    }
}
fn error_response(error: String) -> Response {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": error }))).into_response()
}
