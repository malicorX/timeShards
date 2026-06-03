use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation: {0}")]
    Validation(String),
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
}

impl ApiErrorBody {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: u16,
    pub body: ApiErrorBody,
}

impl ApiError {
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: 401,
            body: ApiErrorBody::new("unauthorized", message),
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: 403,
            body: ApiErrorBody::new("forbidden", message),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: 404,
            body: ApiErrorBody::new("not_found", message),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: 400,
            body: ApiErrorBody::new("bad_request", message),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: 500,
            body: ApiErrorBody::new("internal", message),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(self.body)).into_response()
    }
}

impl From<CoreError> for ApiError {
    fn from(value: CoreError) -> Self {
        match value {
            CoreError::Unauthorized => Self::unauthorized("Anmeldung erforderlich"),
            CoreError::Forbidden(msg) => Self::forbidden(msg),
            CoreError::NotFound(msg) => Self::not_found(msg),
            CoreError::Validation(msg) => Self::bad_request(msg),
            CoreError::Other(msg) => Self::internal(msg),
        }
    }
}
