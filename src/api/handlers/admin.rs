use axum::{
    extract::{State, Json as JsonExtractor},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::errors::TokenGatingError;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ConfigureResourceRequest {
    pub name: String,
    pub required_mint: String,
    pub level_thresholds: Option<Vec<u64>>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigureClaimRequest {
    pub resource_name: String,
    pub start_time: u64,
    pub end_time: u64,
    pub required_level: u8,
}

#[derive(Debug, Deserialize)]
pub struct AddAdminRequest {
    pub resource_name: String,
    pub admin_pubkey: String,
}

pub async fn configure_resource(
    State(state): State<AppState>,
    JsonExtractor(request): JsonExtractor<ConfigureResourceRequest>,
) -> Result<Json<serde_json::Value>, TokenGatingError> {
    // First configure the resource
    let signature = state.token_gating_client
        .configure_resource(request.name.clone(), &request.required_mint)
        .await?;

    // Then configure the levels if provided
    let level_signature = if let Some(thresholds) = request.level_thresholds {
        Some(state.token_gating_client
            .configure_levels(request.name, thresholds)
            .await?)
    } else {
        None
    };

    Ok(Json(json!({
        "status": "success",
        "resource_signature": signature,
        "levels_signature": level_signature
    })))
}

pub async fn configure_claim(
    State(state): State<AppState>,
    JsonExtractor(request): JsonExtractor<ConfigureClaimRequest>,
) -> Result<Json<serde_json::Value>, TokenGatingError> {
    let signature = state.token_gating_client
        .configure_claim(
            request.resource_name,
            request.start_time,
            request.end_time,
            request.required_level,
        )
        .await?;

    Ok(Json(json!({
        "status": "success",
        "signature": signature
    })))
}

pub async fn add_admin(
    State(state): State<AppState>,
    JsonExtractor(request): JsonExtractor<AddAdminRequest>,
) -> Result<Json<serde_json::Value>, TokenGatingError> {
    // Call the async add_admin method
    let signature = state.token_gating_client
        .add_admin(request.resource_name.clone(), &request.admin_pubkey)
        .await
        .map_err(|e| {
            tracing::error!("Error adding admin: {:?}", e);
            TokenGatingError::TransactionError(e.to_string())
        })?;

    Ok(Json(json!({
        "status": "success",
        "signature": signature,
        "resource": request.resource_name
    })))
} 