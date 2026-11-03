use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub retry_after: Option<u64>,
}

#[derive(Serialize)]
struct Envelope<'a> {
    message: &'a str,
    code: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_after: Option<u64>,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            retry_after: None,
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }

    pub fn session_exists() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            "session_exists",
            "session already exists",
        )
    }

    pub fn job_exists() -> Self {
        Self::new(StatusCode::CONFLICT, "job_exists", "job already exists")
    }

    pub fn file_not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "file_not_found", message)
    }

    pub fn no_video_stream() -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "no_video_stream",
            "file has no video stream",
        )
    }

    pub fn transcode_limit(retry_after: u64) -> Self {
        Self {
            retry_after: Some(retry_after),
            ..Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "transcode_limit",
                "all transcode slots are in use",
            )
        }
    }

    pub fn transcode_unavailable(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "transcode_unavailable",
            message,
        )
    }

    pub fn ffmpeg_unavailable(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "ffmpeg_unavailable",
            message,
        )
    }

    pub fn queue_full(retry_after: u64) -> Self {
        Self {
            retry_after: Some(retry_after),
            ..Self::new(
                StatusCode::TOO_MANY_REQUESTS,
                "queue_full",
                "job queue is full",
            )
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    pub fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "missing or invalid credentials",
        )
    }

    pub fn forbidden() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "token is invalid or expired",
        )
    }

    pub fn ffmpeg_failed(message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, "ffmpeg_failed", message)
    }

    pub fn session_restarted() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "session_restarted",
            "session was restarted while the request waited",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Envelope {
            message: &self.message,
            code: self.code,
            retry_after: self.retry_after,
        };
        (self.status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn envelope_shape() {
        let res = ApiError::transcode_limit(10).into_response();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["code"], "transcode_limit");
        assert_eq!(v["retry_after"], 10);
        assert!(v["message"].is_string());
    }

    #[test]
    fn retry_after_omitted_by_default() {
        let body = serde_json::to_value(Envelope {
            message: "m",
            code: "not_found",
            retry_after: None,
        })
        .unwrap();
        assert!(body.get("retry_after").is_none());
    }
}
