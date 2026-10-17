// TODO: rename this file to errors.rs

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use tempfile::PersistError;
use thiserror::Error;

#[derive(Error, Debug)]
#[error("{public_message}")]
pub struct ErrorStatus {
    code: StatusCode,
    public_message: String,
    log_message: String,
}

impl From<StatusCode> for ErrorStatus {
    fn from(code: StatusCode) -> Self {
        Self {
            code,
            public_message: "an error occurred".to_string(),
            log_message: "no error message provided".to_string(),
        }
    }
}

impl From<tokio::task::JoinError> for ErrorStatus {
    fn from(err: tokio::task::JoinError) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            public_message: "internal server error".to_string(),
            log_message: err.to_string(),
        }
    }
}

impl From<PersistError> for ErrorStatus {
    fn from(err: PersistError) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            public_message: "internal server error".to_string(),
            log_message: err.to_string(),
        }
    }
}

impl From<std::io::Error> for ErrorStatus {
    fn from(err: std::io::Error) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            public_message: "internal server error".to_string(),
            log_message: err.to_string(),
        }
    }
}

impl From<(StatusCode, &'static str)> for ErrorStatus {
    fn from((code, message): (StatusCode, &'static str)) -> Self {
        Self {
            code,
            public_message: message.to_string(),
            log_message: message.to_string(),
        }
    }
}

impl From<(StatusCode, &'static str, &'static str)> for ErrorStatus {
    fn from((code, message, log_message): (StatusCode, &'static str, &'static str)) -> Self {
        Self {
            code,
            public_message: message.to_string(),
            log_message: log_message.to_string(),
        }
    }
}

impl From<sqlx::Error> for ErrorStatus {
    fn from(err: sqlx::Error) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            public_message: "internal server error".to_string(),
            log_message: err.to_string(),
        }
    }
}

impl IntoResponse for ErrorStatus {
    fn into_response(self) -> Response {
        tracing::error!(code = %self.code, "{}", self.log_message);
        (self.code, self.public_message).into_response()
    }
}
