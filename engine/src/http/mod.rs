mod control;
mod data;

use axum::Router;
use axum::extract::rejection::PathRejection;
use axum::extract::{FromRequestParts, Path, Request, State};
use axum::http::request::Parts;
use axum::http::{Method, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, patch, post};
use serde::de::DeserializeOwned;
use tower_http::cors::{Any, CorsLayer};

use crate::AppState;
use crate::error::ApiError;

pub fn router(state: AppState) -> Router {
    let control = Router::new()
        .route("/health", get(control::health))
        .route(
            "/sessions",
            get(control::list_sessions).post(control::start_session),
        )
        .route(
            "/sessions/{id}",
            patch(control::patch_session).delete(control::delete_session),
        )
        .route("/decisions", post(control::decisions))
        .route("/jobs", get(control::list_jobs).post(control::submit_job))
        .route(
            "/jobs/{id}",
            get(control::job_status).delete(control::cancel_job),
        )
        .route("/jobs/{id}/files/{*name}", get(control::job_file))
        .method_not_allowed_fallback(method_not_allowed)
        .layer(middleware::from_fn_with_state(state.clone(), bearer));

    let data = Router::new()
        .route("/hls/{id}/{file}", get(data::hls))
        .route("/file/{id}", get(data::file))
        .route("/dl/{id}", get(data::download))
        .method_not_allowed_fallback(method_not_allowed)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::HEAD, Method::OPTIONS])
                .allow_headers([header::RANGE])
                .expose_headers([
                    header::CONTENT_LENGTH,
                    header::CONTENT_RANGE,
                    header::ACCEPT_RANGES,
                ]),
        );

    Router::new()
        .nest("/v1", control)
        .merge(data)
        .fallback(|| async { ApiError::not_found("no such route") })
        .with_state(state)
}

async fn method_not_allowed() -> ApiError {
    ApiError::method_not_allowed()
}

async fn bearer(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let presented = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(ApiError::unauthorized)?;
    if !constant_time_eq(presented.as_bytes(), &state.secret) {
        return Err(ApiError::unauthorized());
    }
    Ok(next.run(req).await)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        diff |= usize::from(a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0));
    }
    diff == 0
}

pub struct Params<T>(pub T);

impl<T, S> FromRequestParts<S> for Params<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        Path::<T>::from_request_parts(parts, state)
            .await
            .map(|Path(v)| Self(v))
            .map_err(|e: PathRejection| ApiError::invalid_request(e.body_text()))
    }
}
