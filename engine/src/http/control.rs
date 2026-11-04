use axum::Json;
use axum::body::Bytes;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::AppState;
use crate::analysis::ANALYSIS_VERSION;
use crate::decision::EncoderSet;
use crate::error::ApiError;
use crate::http::Params;
use crate::jobs::{JobRequest, JobSpec, JobState, JobStatus, Priority};
use crate::session::{PatchRequest, SessionInfo, SessionResponse, StartRequest};

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
    dovi_tool: bool,
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
        dovi_tool: s.hw.encoders.dovi_tool,
        encoders: encoder_names(&s.hw.encoders),
        decoders: s.hw.decoders(),
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
    Params(id): Params<String>,
    body: Bytes,
) -> Result<Json<SessionResponse>, ApiError> {
    let req: PatchRequest = parse(&body)?;
    Ok(Json(s.sessions.patch(&id, req).await?))
}

pub async fn delete_session(
    State(s): State<AppState>,
    Params(id): Params<String>,
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

pub async fn submit_job(State(s): State<AppState>, body: Bytes) -> Result<StatusCode, ApiError> {
    let v: serde_json::Value = parse(&body)?;
    if let Some(t) = v.get("type").and_then(|t| t.as_str())
        && !JobSpec::knows(t)
    {
        return Err(ApiError::invalid_request(
            "job type not supported in this version",
        ));
    }
    let req: JobRequest =
        serde_json::from_value(v).map_err(|e| ApiError::invalid_request(e.to_string()))?;
    s.jobs.submit(req)?;
    Ok(StatusCode::ACCEPTED)
}

pub async fn job_status(
    State(s): State<AppState>,
    Params(id): Params<String>,
) -> Result<Json<JobStatus>, ApiError> {
    Ok(Json(s.jobs.status(&id)?))
}

pub async fn job_file(
    State(s): State<AppState>,
    Params((id, name)): Params<(String, String)>,
    req: Request,
) -> Result<Response, ApiError> {
    let path = s.jobs.file(&id, &name)?;
    let Ok(res) = ServeFile::new(path).oneshot(req).await;
    Ok(res.into_response())
}

pub async fn cancel_job(
    State(s): State<AppState>,
    Params(id): Params<String>,
) -> Result<StatusCode, ApiError> {
    s.jobs.cancel(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct JobEntry {
    job_id: String,
    state: JobState,
    priority: Priority,
}

#[derive(Serialize)]
pub struct JobList {
    jobs: Vec<JobEntry>,
}

pub async fn list_jobs(State(s): State<AppState>) -> Json<JobList> {
    Json(JobList {
        jobs: s
            .jobs
            .list()
            .into_iter()
            .map(|(job_id, state, priority)| JobEntry {
                job_id,
                state,
                priority,
            })
            .collect(),
    })
}
