use axum::{
    extract::State,
    Json,
};
use serde_json::json;

use crate::errors::TokenGatingError;

pub async fn submit_transaction() -> Result<Json<serde_json::Value>, TokenGatingError> {
    Ok(Json(json!({"status": "transaction submitted"})))
} 