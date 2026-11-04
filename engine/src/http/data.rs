use std::collections::HashMap;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderValue, Method, header};
use axum::response::{IntoResponse, Response};
use http_body::{Frame, SizeHint};
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::AppState;
use crate::error::ApiError;
use crate::session::{OpenStream, Served};
use crate::token::{Scope, verify};

const PLAYLIST_TYPE: &str = "application/vnd.apple.mpegurl";

fn check_token(
    s: &AppState,
    scope: Scope,
    id: &str,
    q: &HashMap<String, String>,
) -> Result<(), ApiError> {
    let exp = q.get("exp").and_then(|e| e.parse::<u64>().ok());
    let sig = q.get("sig");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    match (exp, sig) {
        (Some(exp), Some(sig)) if verify(&s.secret, scope, id, exp, sig, now) => Ok(()),
        _ => Err(ApiError::forbidden()),
    }
}

fn hls_file_ok(file: &str) -> bool {
    if matches!(file, "master.m3u8" | "media.m3u8" | "init.mp4") {
        return true;
    }
    file.strip_prefix("seg_")
        .and_then(|r| r.strip_suffix(".m4s"))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

struct Counted {
    inner: Body,
    stream: OpenStream,
}

impl http_body::Body for Counted {
    type Data = <Body as http_body::Body>::Data;
    type Error = <Body as http_body::Body>::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let polled = Pin::new(&mut self.inner).poll_frame(cx);
        if let Poll::Ready(Some(Ok(frame))) = &polled
            && let Some(data) = frame.data_ref()
        {
            self.stream.add_bytes(data.len() as u64);
        }
        polled
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

async fn serve_file(
    s: &AppState,
    id: &str,
    path: std::path::PathBuf,
    req: Request,
    content_type: Option<&'static str>,
) -> Response {
    let head = req.method() == Method::HEAD;
    let Ok(mut res) = ServeFile::new(path)
        .oneshot(req)
        .await
        .map(IntoResponse::into_response);
    if let Some(ct) = content_type {
        res.headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(ct));
    }
    if !head
        && res.status().is_success()
        && let Some(stream) = s.sessions.open_stream(id)
    {
        res = res.map(|inner| Body::new(Counted { inner, stream }));
    }
    res
}

pub async fn hls(
    State(s): State<AppState>,
    Path((id, file)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
    req: Request,
) -> Result<Response, ApiError> {
    check_token(&s, Scope::Hls, &id, &q)?;
    if !hls_file_ok(&file) {
        return Err(ApiError::not_found("no such file"));
    }
    match s.sessions.fetch_hls(&id, &file).await? {
        Served::Text(text) => {
            s.sessions.add_bytes(&id, text.len() as u64);
            let mut res = Response::new(Body::from(text));
            res.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(PLAYLIST_TYPE),
            );
            Ok(res)
        }
        Served::File(path) => Ok(serve_file(&s, &id, path, req, Some("video/mp4")).await),
    }
}

pub async fn file(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    req: Request,
) -> Result<Response, ApiError> {
    check_token(&s, Scope::Hls, &id, &q)?;
    let path = s.sessions.direct_path(&id).await?;
    Ok(serve_file(&s, &id, path, req, None).await)
}

pub async fn download(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    req: Request,
) -> Result<Response, ApiError> {
    check_token(&s, Scope::Dl, &id, &q)?;
    let path = s.jobs.download_path(&id)?;
    if !path.is_file() {
        return Err(ApiError::not_found("no such file"));
    }
    let Ok(res) = ServeFile::new(path).oneshot(req).await;
    Ok(res.into_response())
}
