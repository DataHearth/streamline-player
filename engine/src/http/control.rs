use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::AppState;
use crate::decision::EncoderSet;
use crate::error::ApiError;
use crate::session::{PatchRequest, SessionInfo, SessionResponse, StartRequest};

const ANALYSIS_VERSION: u32 = 1;

fn parse<T: DeserializeOwned>(body: &Bytes) -> Result<T, ApiError> {
    serde_json::from_slice(body).map_err(|e| ApiError::invalid_request(e.to_string()))
}

#[derive(Serialize)]
pub struct Health {
    api_version: u32,
    version: &'static str,
    analysis_version: u32,
    ffmpeg_version: Option<String>,
    backend: &'static str,
    untested: bool,
    vaapi: bool,
    libplacebo: bool,
    encoders: Vec<&'static str>,
    decoders: Vec<&'static str>,
}

fn encoder_names(set: &EncoderSet) -> Vec<&'static str> {
    [("h264", set.h264), ("hevc", set.hevc), ("av1", set.av1)]
        .into_iter()
        .filter_map(|(name, on)| on.then_some(name))
        .collect()
}

pub async fn health(State(s): State<AppState>) -> Json<Health> {
    Json(Health {
        api_version: 1,
        version: env!("CARGO_PKG_VERSION"),
        analysis_version: ANALYSIS_VERSION,
        ffmpeg_version: s.ffmpeg_version.clone(),
        backend: crate::session::backend_name(s.hw.backend),
        untested: s.hw.untested,
        vaapi: s.hw.backend == crate::hw::Backend::Vaapi,
        libplacebo: s.hw.libplacebo,
        encoders: encoder_names(&s.hw.encoders),
        decoders: vec![],
    })
}

pub async fn start_session(
    State(s): State<AppState>,
    body: Bytes,
) -> Result<Json<SessionResponse>, ApiError> {
    Ok(Json(s.sessions.start(parse::<StartRequest>(&body)?).await?))
}

pub async fn patch_session(
    State(s): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<SessionResponse>, ApiError> {
    let req: PatchRequest = parse(&body)?;
    Ok(Json(s.sessions.patch(&id, req).await?))
}

pub async fn delete_session(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    s.sessions.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
pub struct SessionList {
    sessions: Vec<SessionInfo>,
}

pub async fn list_sessions(State(s): State<AppState>) -> Json<SessionList> {
    Json(SessionList {
        sessions: s.sessions.list(),
    })
}
