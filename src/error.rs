use axum::{http::StatusCode, response::IntoResponse, response::Response};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RouterError {
    #[error("Config error: {0}")]
    Config(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: status={status}, body={body}")]
    HttpError { status: u16, body: String },

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Context length exceeded")]
    ContextLengthExceeded,

    #[error("All providers failed")]
    AllProvidersFailed,

    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

impl IntoResponse for RouterError {
    fn into_response(self) -> Response {
        let status = match &self {
            RouterError::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
            RouterError::Network(_) => StatusCode::BAD_GATEWAY,
            RouterError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            RouterError::HttpError { status, .. } => {
                StatusCode::from_u16(*status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            }
            RouterError::RateLimit => StatusCode::TOO_MANY_REQUESTS,
            RouterError::ContextLengthExceeded => StatusCode::BAD_REQUEST,
            RouterError::AllProvidersFailed => StatusCode::BAD_GATEWAY,
            RouterError::ProviderNotFound(_) => StatusCode::NOT_FOUND,
            RouterError::NotFound(_) => StatusCode::NOT_FOUND,
        };

        (status, format!("{:?}", self)).into_response()
    }
}

pub type Result<T> = std::result::Result<T, RouterError>;
