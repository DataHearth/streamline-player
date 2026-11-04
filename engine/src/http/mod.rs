mod control;
mod data;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{Method, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, patch};
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
        .layer(middleware::from_fn_with_state(state.clone(), bearer));

    let data = Router::new()
        .route("/hls/{id}/{file}", get(data::hls))
        .route("/file/{id}", get(data::file))
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
        .with_state(state)
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
