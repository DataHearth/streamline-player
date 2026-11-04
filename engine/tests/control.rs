mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use serde_json::json;

#[tokio::test]
async fn health_requires_bearer() {
    let e = env();
    let res = e.get("/v1/health").await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let res = e
        .send(
            Request::get("/v1/health")
                .header("authorization", "Bearer nope")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let v: serde_json::Value = serde_json::from_slice(&body_bytes(res).await).unwrap();
    assert_eq!(v["code"], "unauthorized");

    let (status, v) = e.api("GET", "/v1/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["api_version"], 1);
    assert_eq!(v["analysis_version"], 1);
    assert_eq!(v["backend"], "cpu");
    assert_eq!(v["vaapi"], false);
    assert_eq!(v["encoders"], json!(["h264", "hevc", "av1"]));
    assert_eq!(v["decoders"], json!([]));
}

#[tokio::test]
async fn unknown_capability_field_is_400() {
    let e = env();
    let mut body = start_body(&id(1), &e.root.join("in.mkv"), "hevc", "mp4");
    body["capabilities"]["foo"] = json!(1);
    let (status, v) = e.api("POST", "/v1/sessions", Some(body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(v["code"], "invalid_request");
}

#[tokio::test]
async fn malformed_body_is_400() {
    let e = env();
    let (status, v) = e
        .api("POST", "/v1/sessions", Some(json!({"nope": true})))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(v["code"], "invalid_request");
}

#[tokio::test]
async fn bad_session_id_is_400() {
    let e = env();
    let body = start_body("xyz", &e.root.join("in.mkv"), "hevc", "mp4");
    let (status, v) = e.api("POST", "/v1/sessions", Some(body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(v["code"], "invalid_request");
    let (status, _) = e.api("DELETE", "/v1/sessions/xyz", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[cfg(unix)]
#[tokio::test]
async fn duplicate_is_409() {
    let e = env();
    std::fs::write(e.root.join("in.mkv"), b"x").unwrap();
    let body = start_body(&id(2), &e.root.join("in.mkv"), "hevc", "mp4");
    let (status, v) = e.api("POST", "/v1/sessions", Some(body.clone())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["mode"], "remux");
    let (status, v) = e.api("POST", "/v1/sessions", Some(body)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(v["code"], "session_exists");
}

#[cfg(unix)]
#[tokio::test]
async fn session_lifecycle() {
    let e = env();
    std::fs::write(e.root.join("in.mkv"), b"x").unwrap();
    let sid = id(3);
    let body = start_body(&sid, &e.root.join("in.mkv"), "hevc", "mp4");
    e.api("POST", "/v1/sessions", Some(body)).await;

    let (status, v) = e.api("GET", "/v1/sessions", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["sessions"][0]["session_id"], sid.as_str());

    let (status, v) = e
        .api(
            "PATCH",
            &format!("/v1/sessions/{sid}"),
            Some(json!({"position": 30.0})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["mode"], "remux");

    let (status, _) = e.api("DELETE", &format!("/v1/sessions/{sid}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, v) = e.api("DELETE", &format!("/v1/sessions/{sid}"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(v["code"], "not_found");
}
