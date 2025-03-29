use thiserror::Error;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Error, Debug)]
pub enum TokenGatingError {
    // Configuration Errors
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    // Blockchain Interaction Errors
    #[error("Solana RPC error: {0}")]
    SolanaRpcError(String),

    #[error("Transaction submission failed: {0}")]
    TransactionError(String),

    // Access Control Errors
    #[error("Insufficient token balance for resource access")]
    InsufficientTokenBalance,

    #[error("Resource access denied")]
    AccessDenied,

    #[error("Invalid resource configuration")]
    InvalidResourceConfig,
    
    #[error("Resource not found")]
    ResourceNotFound,

    // Admin Errors
    #[error("Unauthorized admin action")]
    UnauthorizedAdmin,

    // Claim Window Errors
    #[error("Claim window is closed")]
    ClaimWindowClosed,

    #[error("Multiple claims not allowed")]
    MultipleClaims,

    // Generic Errors
    #[error("Internal server error: {0}")]
    InternalServerError(String),

    // New Connection Error
    #[error("Connection error: {0}")]
    ConnectionError(String),
}

impl TokenGatingError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            TokenGatingError::ConfigurationError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            TokenGatingError::SolanaRpcError(_) => StatusCode::BAD_GATEWAY,
            TokenGatingError::TransactionError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            TokenGatingError::InsufficientTokenBalance => StatusCode::FORBIDDEN,
            TokenGatingError::AccessDenied => StatusCode::FORBIDDEN,
            TokenGatingError::InvalidResourceConfig => StatusCode::BAD_REQUEST,
            TokenGatingError::ResourceNotFound => StatusCode::NOT_FOUND,
            TokenGatingError::UnauthorizedAdmin => StatusCode::UNAUTHORIZED,
            TokenGatingError::ClaimWindowClosed => StatusCode::FORBIDDEN,
            TokenGatingError::MultipleClaims => StatusCode::FORBIDDEN,
            TokenGatingError::InternalServerError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            TokenGatingError::ConnectionError(_) => StatusCode::BAD_GATEWAY,
        }
    }
}

impl IntoResponse for TokenGatingError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({
            "error": self.to_string(),
            "code": self.status_code().as_u16()
        });

        (self.status_code(), axum::Json(body)).into_response()
    }
}

// Conversion from other error types
impl From<solana_client::client_error::ClientError> for TokenGatingError {
    fn from(err: solana_client::client_error::ClientError) -> Self {
        TokenGatingError::SolanaRpcError(err.to_string())
    }
}

impl From<std::io::Error> for TokenGatingError {
    fn from(err: std::io::Error) -> Self {
        TokenGatingError::InternalServerError(err.to_string())
    }
} 