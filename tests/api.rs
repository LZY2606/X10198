mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use phase_weave::{api, App};
use std::sync::Arc;
use tower::ServiceExt;

async fn body_string(resp: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn index_shows_phase_weave_title() {
    let app = Arc::new(App::open(":memory:").unwrap());
    let router = api::router(app);
    let resp = router
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("相位织图"));
}

#[tokio::test]
async fn import_view_and_stale_decide_flows() {
    let app = Arc::new(App::open(":memory:").unwrap());
    let router = api::router(app.clone());

    let post = |uri: &str, payload: serde_json::Value| {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap()
    };

    // Import version 1.
    let resp = router.clone().oneshot(post("/api/import", serde_json::to_value(trio_bundle()).unwrap())).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // State contains the phased block.
    let resp = router
        .clone()
        .oneshot(Request::builder().uri("/api/state?branch=main").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let state: serde_json::Value = serde_json::from_str(&body_string(resp).await).unwrap();
    assert_eq!(state["input_version"], 1);
    assert!(state["blocks"].as_array().unwrap().iter().any(|b| b["id"] == "B-3-chr1-101"));

    // Import another empty batch -> version 2.
    let empty = serde_json::json!({"samples":[],"relationships":[],"variants":[],
        "observations":[],"read_links":[]});
    router.clone().oneshot(post("/api/import", empty)).await.unwrap();

    // Stale decision against version 1 -> 409.
    let stale = serde_json::json!({"branch":"main","expected_version":1,
        "decision":{"action":"mark_relationship","relationship_id":11,"status":"to_confirm"}});
    let resp = router.clone().oneshot(post("/api/decide", stale)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // Fresh decision -> 200 and visible in state.
    let fresh = serde_json::json!({"branch":"main","expected_version":2,
        "decision":{"action":"mark_relationship","relationship_id":11,"status":"to_confirm"}});
    let resp = router.clone().oneshot(post("/api/decide", fresh)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = router
        .oneshot(Request::builder().uri("/api/state?branch=main").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let state: serde_json::Value = serde_json::from_str(&body_string(resp).await).unwrap();
    assert_eq!(state["events"].as_array().unwrap().len(), 1);
}
