pub mod analysis;
pub mod args;
pub mod caps;
pub mod config;
pub mod decision;
pub mod error;
pub mod http;
pub mod hw;
pub mod jobs;
pub mod playlist;
pub mod session;
pub mod telemetry;
pub mod token;

use std::sync::Arc;

use axum::Router;

use config::Config;
use hw::Hw;
use jobs::Jobs;
use session::Sessions;

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub secret: Arc<Vec<u8>>,
    pub hw: Arc<Hw>,
    pub ffmpeg_version: Option<String>,
    pub sessions: Sessions,
    pub jobs: Jobs,
}

pub fn app(state: AppState) -> Router {
    http::router(state)
}
