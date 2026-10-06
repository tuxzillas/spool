// i made this because I didnt want to use Response<Body>>
// ...
// is that a dumb reason?
// yes.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use tempfile::PersistError;

#[derive(Debug)]
pub struct ErrorStatus {
    code: StatusCode,
    message: String,
}

impl From<StatusCode> for ErrorStatus {
    fn from(code: StatusCode) -> Self {
        Self {
            code,
            message: "no error message provided".to_string(),
        }
    }
}

impl From<tokio::task::JoinError> for ErrorStatus {
    fn from(err: tokio::task::JoinError) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            message: err.to_string(),
        }
    }
}

impl From<(StatusCode, PersistError)> for ErrorStatus {
    fn from((code, err): (StatusCode, PersistError)) -> Self {
        Self {
            code,
            message: err.to_string(),
        }
    }
}

impl From<(StatusCode, &'static str)> for ErrorStatus {
    fn from((code, message): (StatusCode, &'static str)) -> Self {
        Self {
            code,
            message: message.to_string(),
        }
    }
}

impl From<sqlx::Error> for ErrorStatus {
    fn from(err: sqlx::Error) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            message: err.to_string(),
        }
    }
}

impl IntoResponse for ErrorStatus {
    fn into_response(self) -> Response {
        println!("ERROR: {} {}", self.code, self.message);

        (self.code, self.message).into_response()
    }
}
