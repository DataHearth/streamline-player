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

fn job_body(n: u8, kind: &str) -> serde_json::Value {
    json!({"job_id": id(n), "type": kind, "priority": "background", "path": "/m/a"})
}

#[cfg(unix)]
async fn wait_done(e: &Env, n: u8) -> serde_json::Value {
    for _ in 0..200 {
        let (status, v) = e.api("GET", &format!("/v1/jobs/{}", id(n)), None).await;
        if status == StatusCode::OK && (v["state"] == "done" || v["state"] == "failed") {
            return v;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("job did not finish");
}

#[cfg(unix)]
#[tokio::test]
async fn job_lifecycle() {
    let e = env();
    let (status, _) = e
        .api("POST", "/v1/jobs", Some(job_body(1, "analyze")))
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let (status, v) = e
        .api("POST", "/v1/jobs", Some(job_body(1, "analyze")))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(v["code"], "job_exists");

    let v = wait_done(&e, 1).await;
    assert_eq!(v["state"], "done");
    assert_eq!(v["result"]["version"], 1);

    let (status, v) = e.api("GET", "/v1/jobs", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["jobs"], json!([]));

    let (status, _) = e.api("DELETE", &format!("/v1/jobs/{}", id(1)), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, v) = e.api("GET", &format!("/v1/jobs/{}", id(1)), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(v["code"], "not_found");
}

#[tokio::test]
async fn unsupported_type_400() {
    let e = env();
    let (status, v) = e
        .api("POST", "/v1/jobs", Some(json!({"type": "trickplay"})))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(v["code"], "invalid_request");
    assert_eq!(v["message"], "job type not supported in this version");

    let (status, v) = e
        .api("POST", "/v1/jobs", Some(json!({"job_id": "nope"})))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(v["code"], "invalid_request");
}

#[tokio::test]
async fn bad_job_id_is_400() {
    let e = env();
    let mut body = job_body(1, "analyze");
    body["job_id"] = json!("xyz");
    let (status, _) = e.api("POST", "/v1/jobs", Some(body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = e.api("GET", "/v1/jobs/xyz", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn job_file_traversal_404() {
    let e = env();
    let jid = id(7);
    let dir = e.root.join("jobs").join(&jid);
    std::fs::create_dir_all(dir.join("fonts")).unwrap();
    std::fs::write(
        dir.join("done.json"),
        r#"{"state":"done","result":{},"error":null}"#,
    )
    .unwrap();
    std::fs::write(dir.join("fonts/A.ttf"), b"font").unwrap();
    std::fs::write(e.root.join("secret.txt"), b"secret").unwrap();

    let res = e
        .get_authed(&format!("/v1/jobs/{jid}/files/fonts/A.ttf"))
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_bytes(res).await, b"font");

    for name in [
        "../done.json",
        "%2e%2e/done.json",
        "..%2Fdone.json",
        "../../secret.txt",
        "..%2F..%2Fsecret.txt",
        "%2Fetc%2Fpasswd",
        "/etc/passwd",
        "fonts%5C..%5Cx",
        "fonts/../done.json",
        "done.json",
        "",
    ] {
        let res = e.get_authed(&format!("/v1/jobs/{jid}/files/{name}")).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{name:?}");
    }
}

#[tokio::test]
async fn jobs_require_bearer() {
    let e = env();
    let res = e.get(&format!("/v1/jobs/{}", id(1))).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
