use axum::{
    response::{Response, IntoResponse},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Authentication error: {0}")]
    Auth(String),
    
    #[error("Authorization error: {0}")]
    Forbidden(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Bad request: {0}")]
    BadRequest(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("External service error: {0}")]
    External(String),
    
    #[error("Payment processing error: {0}")]
    Payment(String),
    
    #[error("Internal server error: {0}")]
    Internal(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
}

#[derive(Serialize, Deserialize)]
struct ErrorResponse {
    error: String,
    message: String,
    status_code: u16,
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = match &self {
            ServerError::Auth(_) => StatusCode::UNAUTHORIZED,
            ServerError::Forbidden(_) => StatusCode::FORBIDDEN,
            ServerError::NotFound(_) => StatusCode::NOT_FOUND,
            ServerError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ServerError::Validation(_) => StatusCode::BAD_REQUEST,
            ServerError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::External(_) => StatusCode::BAD_GATEWAY,
            ServerError::Payment(_) => StatusCode::BAD_REQUEST,
            ServerError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        
        let error_type = match &self {
            ServerError::Auth(_) => "authentication_error",
            ServerError::Forbidden(_) => "authorization_error",
            ServerError::NotFound(_) => "not_found",
            ServerError::BadRequest(_) => "bad_request",
            ServerError::Validation(_) => "validation_error",
            ServerError::Database(_) => "database_error",
            ServerError::External(_) => "external_service_error",
            ServerError::Payment(_) => "payment_error",
            ServerError::Internal(_) => "internal_server_error",
            ServerError::Config(_) => "configuration_error",
        };
        
        let body = Json(ErrorResponse {
            error: error_type.to_string(),
            message: self.to_string(),
            status_code: status.as_u16(),
        });
        
        (status, body).into_response()
    }
}

impl From<sqlx::Error> for ServerError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => ServerError::NotFound("Record not found".to_string()),
            _ => ServerError::Database(err.to_string()),
        }
    }
}

impl From<jsonwebtoken::errors::Error> for ServerError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        ServerError::Auth(format!("JWT error: {}", err))
    }
}

impl From<reqwest::Error> for ServerError {
    fn from(err: reqwest::Error) -> Self {
        ServerError::External(format!("HTTP request error: {}", err))
    }
}

impl From<config::ConfigError> for ServerError {
    fn from(err: config::ConfigError) -> Self {
        ServerError::Config(format!("Configuration error: {}", err))
    }
}