mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::*;

#[cfg(unix)]
async fn remux_session(e: &Env, n: u8) -> String {
    std::fs::write(e.root.join("in.mkv"), b"x").unwrap();
    let sid = id(n);
    let body = start_body(&sid, &e.root.join("in.mkv"), "hevc", "mp4");
    let (status, _) = e.api("POST", "/v1/sessions", Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    sid
}

#[cfg(unix)]
#[tokio::test]
async fn hls_requires_token() {
    let e = env();
    let sid = remux_session(&e, 1).await;

    let res = e.get(&format!("/hls/{sid}/master.m3u8")).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let expired = token_query(&sid, now() - 10);
    let res = e.get(&format!("/hls/{sid}/master.m3u8?{expired}")).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let other = token_query(&id(9), now() + 3600);
    let res = e.get(&format!("/hls/{sid}/master.m3u8?{other}")).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let q = valid_query(&sid);
    let res = e.get(&format!("/hls/{sid}/master.m3u8?{q}")).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()[header::CONTENT_TYPE],
        "application/vnd.apple.mpegurl"
    );
    let text = String::from_utf8(body_bytes(res).await).unwrap();
    assert!(text.contains("media.m3u8"));

    let res = e.get(&format!("/hls/{sid}/seg_0.m4s?{q}")).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "video/mp4");
    assert!(res.headers().contains_key(header::ACCEPT_RANGES));
}

#[cfg(unix)]
#[tokio::test]
async fn playlists_carry_the_token_to_every_uri() {
    let e = env();
    let sid = remux_session(&e, 3).await;
    let q = valid_query(&sid);
    let text = |file: &'static str| {
        let (e, sid, q) = (&e, &sid, &q);
        async move {
            let res = e.get(&format!("/hls/{sid}/{file}?{q}")).await;
            String::from_utf8(body_bytes(res).await).unwrap()
        }
    };

    let master = text("master.m3u8").await;
    assert!(master.contains(&format!("\nmedia.m3u8?{q}\n")), "{master}");
    let media = text("media.m3u8").await;
    assert!(media.contains(&format!("URI=\"init.mp4?{q}\"")), "{media}");
    let uris: Vec<_> = media
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert!(!uris.is_empty());
    assert!(
        uris.iter().all(|l| l.ends_with(&format!(".m4s?{q}"))),
        "{media}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn hls_path_traversal_404() {
    let e = env();
    let sid = remux_session(&e, 2).await;
    let q = valid_query(&sid);
    for file in [
        "..%2Fx",
        "..%2F..%2Fetc%2Fpasswd",
        "seg_1.m4s%2F..%2F..%2Fetc",
        "seg_1.m4s/../../etc",
        "..",
        "C:%5Cx",
        "..%5Cx",
        "seg_1.m4s.bak",
        "ffmpeg_0.m3u8",
        "init_0.mp4",
    ] {
        let res = e.get(&format!("/hls/{sid}/{file}?{q}")).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{file}");
    }
    let res = e.get(&format!("/hls/{sid}/%2e%2e/x?{q}")).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn hls_unknown_session_is_404() {
    let e = env();
    let sid = id(3);
    let q = valid_query(&sid);
    let res = e.get(&format!("/hls/{sid}/master.m3u8?{q}")).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[cfg(unix)]
#[tokio::test]
async fn file_range() {
    let e = env();
    let path = e.root.join("big.mp4");
    std::fs::write(&path, vec![7u8; 10 * 1024]).unwrap();
    let sid = id(4);
    let mut body = start_body(&sid, &path, "hevc", "mp4");
    body["analysis"]["container"] = "mp4".into();
    let (status, v) = e.api("POST", "/v1/sessions", Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["mode"], "direct");

    let q = valid_query(&sid);
    let res = e
        .send(
            Request::get(format!("/file/{sid}?{q}"))
                .header(header::RANGE, "bytes=100-199")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(body_bytes(res).await.len(), 100);
    assert_eq!(e.sessions.list()[0].bytes_served, 100);

    let res = e.get(&format!("/file/{sid}")).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[cfg(unix)]
#[tokio::test]
async fn file_of_non_direct_session_is_404() {
    let e = env();
    let sid = remux_session(&e, 5).await;
    let q = valid_query(&sid);
    let res = e.get(&format!("/file/{sid}?{q}")).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cors_on_data_only() {
    let e = env();
    let sid = id(6);
    let preflight = |uri: String| {
        Request::builder()
            .method("OPTIONS")
            .uri(uri)
            .header(header::ORIGIN, "https://app.example")
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
            .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "range")
            .body(Body::empty())
            .unwrap()
    };
    let res = e.send(preflight(format!("/hls/{sid}/master.m3u8"))).await;
    assert_eq!(res.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    let res = e.send(preflight(format!("/file/{sid}"))).await;
    assert_eq!(res.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    let res = e.send(preflight("/v1/health".into())).await;
    assert!(
        !res.headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
}

fn dl_query(id: &str) -> String {
    let exp = now() + 3600;
    format!(
        "exp={exp}&sig={}",
        engine::token::sign(SECRET.as_bytes(), engine::token::Scope::Dl, id, exp)
    )
}

#[tokio::test]
async fn dl_serves_finished_job_with_dl_scope_only() {
    let e = env();
    let jid = id(5);
    let dir = e.root.join("jobs").join(&jid);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("done.json"), r#"{"state":"done"}"#).unwrap();
    std::fs::write(dir.join("output.mp4"), b"0123456789").unwrap();

    let res = e.get(&format!("/dl/{jid}")).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = e.get(&format!("/dl/{jid}?{}", valid_query(&jid))).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let res = e.get(&format!("/dl/{jid}?{}", dl_query(&jid))).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_bytes(res).await, b"0123456789");

    let res = e
        .send(
            Request::get(format!("/dl/{jid}?{}", dl_query(&jid)))
                .header("range", "bytes=2-4")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(body_bytes(res).await, b"234");

    let res = e.get(&format!("/dl/{}?{}", id(6), dl_query(&id(6)))).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[cfg(unix)]
async fn direct_session(e: &Env, n: u8, len: usize) -> String {
    std::fs::write(e.root.join("big.mkv"), vec![7u8; len]).unwrap();
    let sid = id(n);
    let body = start_body(&sid, &e.root.join("big.mkv"), "hevc", "mkv");
    let (status, v) = e.api("POST", "/v1/sessions", Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["mode"], "direct");
    sid
}

#[cfg(unix)]
async fn settle() {
    for _ in 0..70 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

#[cfg(unix)]
#[tokio::test(start_paused = true)]
async fn open_file_response_is_not_reaped() {
    let e = env();
    let sid = direct_session(&e, 5, 64 * 1024).await;
    let q = valid_query(&sid);
    let res = e.get(&format!("/file/{sid}?{q}")).await;
    assert_eq!(res.status(), StatusCode::OK);
    let reaper = e.sessions.spawn_reaper();

    let past_timeout = std::time::Duration::from_secs(31 * 60);
    tokio::time::advance(past_timeout).await;
    settle().await;
    assert_eq!(e.sessions.list().len(), 1, "reaped mid-stream");

    drop(res);
    tokio::time::advance(past_timeout).await;
    settle().await;
    assert!(
        e.sessions.list().is_empty(),
        "not reaped after the body closed"
    );
    reaper.abort();
}

#[cfg(unix)]
#[tokio::test]
async fn bytes_are_counted_as_sent() {
    let e = env();
    let sid = direct_session(&e, 6, 64 * 1024).await;
    let q = valid_query(&sid);
    let res = e.get(&format!("/file/{sid}?{q}")).await;
    assert_eq!(e.sessions.list()[0].bytes_served, 0);
    let body = body_bytes(res).await;
    assert_eq!(body.len(), 64 * 1024);
    assert_eq!(e.sessions.list()[0].bytes_served, 64 * 1024);
}
