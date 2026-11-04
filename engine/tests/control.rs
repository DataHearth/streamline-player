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
    assert_eq!(v["analysis_version"], engine::analysis::ANALYSIS_VERSION);
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
    assert_eq!(v["result"]["version"], engine::analysis::ANALYSIS_VERSION);

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

#[tokio::test]
async fn boots_and_answers_health() {
    let dir = std::env::temp_dir().join(format!("engine-boot-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let cfg = dir.join("engine.yaml");
    std::fs::write(
        &cfg,
        format!(
            "listen: 127.0.0.1:{port}\nsecret: boot-secret-boot-secret\ntmp_dir: {0}/live\njobs_dir: {0}/jobs\nhw_accel: none\n",
            dir.display()
        ),
    )
    .unwrap();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_engine"))
        .arg("--config")
        .arg(&cfg)
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut status = None;
    for _ in 0..50 {
        if let Ok(mut s) = tokio::net::TcpStream::connect(("127.0.0.1", port)).await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            s.write_all(
                b"GET /v1/health HTTP/1.1\r\nhost: x\r\nauthorization: Bearer boot-secret-boot-secret\r\nconnection: close\r\n\r\n",
            )
            .await
            .unwrap();
            let mut buf = String::new();
            s.read_to_string(&mut buf).await.unwrap();
            status = buf.lines().next().map(str::to_owned);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let _ = child.kill().await;
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(status.as_deref(), Some("HTTP/1.1 200 OK"));
}

#[tokio::test]
async fn health_reports_gpu_decoders_and_version_token() {
    let e = env_with_hw(engine::hw::Hw::new(
        engine::hw::Backend::Vaapi,
        String::new(),
        engine::decision::EncoderSet::SOFTWARE,
        true,
    ));
    let (_, v) = e.api("GET", "/v1/health", None).await;
    assert_eq!(v["decoders"], json!(["vaapi"]));
    assert_eq!(v["vaapi"], true);
    assert_eq!(v["analysis_version"], engine::analysis::ANALYSIS_VERSION);
}

async fn envelope(res: axum::http::Response<Body>) -> (StatusCode, serde_json::Value) {
    let status = res.status();
    let v = serde_json::from_slice(&body_bytes(res).await).expect("body is a json envelope");
    (status, v)
}

#[tokio::test]
async fn unknown_route_is_enveloped_404() {
    let e = env();
    let (status, v) = envelope(e.get("/nope").await).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(v["code"], "not_found");
    let (status, v) = envelope(e.get_authed("/v1/nope").await).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(v["code"], "not_found");
}

#[tokio::test]
async fn wrong_method_is_enveloped_405() {
    let e = env();
    let (status, v) = e.api("PUT", "/v1/health", None).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(v["code"], "method_not_allowed");
    let res = e
        .send(
            Request::post(format!("/hls/{}/master.m3u8", id(1)))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    let (status, v) = envelope(res).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(v["code"], "method_not_allowed");
}

#[tokio::test]
async fn undecodable_query_is_enveloped_403() {
    let e = env();
    for uri in [
        format!("/hls/{}/master.m3u8?exp=%FF", id(1)),
        format!("/file/{}?sig=%FF", id(1)),
        format!("/dl/{}?x=%FF", id(1)),
    ] {
        let (status, v) = envelope(e.get(&uri).await).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
        assert_eq!(v["code"], "forbidden", "{uri}");
    }
}

#[tokio::test]
async fn undecodable_path_is_enveloped() {
    let e = env();
    let (status, v) = envelope(e.get("/hls/%FF/master.m3u8").await).await;
    assert!(status.is_client_error());
    assert!(v["code"].is_string());
    let (status, v) = envelope(e.get_authed("/v1/jobs/%FF").await).await;
    assert!(status.is_client_error());
    assert!(v["code"].is_string());
}

#[cfg(unix)]
#[tokio::test]
async fn corrupt_file_is_422_unreadable_file() {
    let e = env();
    let path = e.root.join("clip.bad");
    std::fs::write(&path, b"x").unwrap();
    let mut body = start_body(&id(9), &path, "hevc", "mp4");
    body.as_object_mut().unwrap().remove("analysis");
    let (status, v) = e.api("POST", "/v1/sessions", Some(body)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(v["code"], "unreadable_file");
}

#[tokio::test]
async fn missing_ffprobe_is_503_ffmpeg_unavailable() {
    let e = env();
    std::fs::remove_file(e.root.join("bin/ffprobe")).ok();
    let path = e.root.join("clip.mkv");
    std::fs::write(&path, b"x").unwrap();
    let mut body = start_body(&id(9), &path, "hevc", "mp4");
    body.as_object_mut().unwrap().remove("analysis");
    let (status, v) = e.api("POST", "/v1/sessions", Some(body)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(v["code"], "ffmpeg_unavailable");
}
