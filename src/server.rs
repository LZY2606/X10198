use crate::model::{DecisionEvent, InputSnapshot};
use crate::seed;
use crate::store::{Store, StoreError};
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    store: Arc<Mutex<Store>>,
}

impl AppState {
    pub fn new(store: Store) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
        }
    }
}

fn api_error(error: StoreError) -> Response {
    let text = error.to_string();
    let status = if text.contains("stale input version") || text.contains("already exists") {
        StatusCode::CONFLICT
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(serde_json::json!({ "error": text }))).into_response()
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn app_js() -> Response {
    (
        [(header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
        include_str!("../static/app.js"),
    )
        .into_response()
}

async fn styles() -> Response {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../static/styles.css"),
    )
        .into_response()
}

async fn state(State(app): State<AppState>) -> Response {
    let store = app.store.lock().expect("store lock");
    match store.analysis() {
        Ok((replayed, analysis)) => Json(serde_json::json!({
            "branch": store.current_branch().unwrap_or_else(|_| "main".to_string()),
            "branches": store.branches().unwrap_or_default(),
            "input": replayed.input,
            "analysis": analysis,
            "events": store.events().unwrap_or_default(),
        }))
        .into_response(),
        Err(error) => api_error(error),
    }
}

async fn import_input(State(app): State<AppState>, Json(input): Json<InputSnapshot>) -> Response {
    let mut store = app.store.lock().expect("store lock");
    match store.import_input(input) {
        Ok(seq) => Json(serde_json::json!({ "seq": seq })).into_response(),
        Err(error) => api_error(error),
    }
}

async fn decide(State(app): State<AppState>, Json(event): Json<DecisionEvent>) -> Response {
    let mut store = app.store.lock().expect("store lock");
    match store.decide(event) {
        Ok(seq) => Json(serde_json::json!({ "seq": seq })).into_response(),
        Err(error) => api_error(error),
    }
}

#[derive(Deserialize)]
struct BranchRequest {
    name: String,
    from: Option<String>,
    at_seq: Option<i64>,
}

async fn create_branch(State(app): State<AppState>, Json(request): Json<BranchRequest>) -> Response {
    let mut store = app.store.lock().expect("store lock");
    let from = request.from.unwrap_or_else(|| store.current_branch().unwrap_or_else(|_| "main".to_string()));
    match store.create_branch(&request.name, &from, request.at_seq) {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(error) => api_error(error),
    }
}

#[derive(Deserialize)]
struct SelectBranchRequest {
    name: String,
}

async fn select_branch(State(app): State<AppState>, Json(request): Json<SelectBranchRequest>) -> Response {
    let mut store = app.store.lock().expect("store lock");
    match store.set_current_branch(&request.name) {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(error) => api_error(error),
    }
}

#[derive(Deserialize)]
struct RollbackRequest {
    at_seq: i64,
}

async fn rollback(State(app): State<AppState>, Json(request): Json<RollbackRequest>) -> Response {
    let mut store = app.store.lock().expect("store lock");
    match store.rollback(request.at_seq) {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(error) => api_error(error),
    }
}

async fn export_bundle(State(app): State<AppState>) -> Response {
    let store = app.store.lock().expect("store lock");
    match store.export_bundle() {
        Ok(bundle) => Json(bundle).into_response(),
        Err(error) => api_error(error),
    }
}

async fn replay(State(app): State<AppState>) -> Response {
    let store = app.store.lock().expect("store lock");
    match store.replay() {
        Ok(state) => Json(state).into_response(),
        Err(error) => api_error(error),
    }
}

pub fn router(store: Store) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/styles.css", get(styles))
        .route("/api/state", get(state))
        .route("/api/import", post(import_input))
        .route("/api/decision", post(decide))
        .route("/api/branch", post(create_branch))
        .route("/api/branch/select", post(select_branch))
        .route("/api/rollback", post(rollback))
        .route("/api/export", get(export_bundle))
        .route("/api/replay", get(replay))
        .with_state(AppState::new(store))
}

pub fn seeded_store(path: &str) -> Result<Store, StoreError> {
    let mut store = Store::open(path)?;
    if store.latest_input_version()?.is_none() {
        store.import_input(seed::demo_input())?;
    }
    Ok(store)
}
