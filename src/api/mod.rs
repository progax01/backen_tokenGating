pub mod handlers;

use crate::errors::TokenGatingError;
use crate::integration::token_gating_client::TokenGatingClient;
use crate::integration::solana_client::SolanaIntegration;
use solana_sdk::signature::Keypair;
use std::sync::Arc;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::AppState;
use rand::random;
use base64::{Engine as _, engine::general_purpose};

#[derive(Debug, Deserialize)]
pub struct ResourceConfig {
    pub name: String,
    pub required_mint: String,
    pub level_thresholds: Option<Vec<u64>>,
}

#[derive(Debug, Deserialize)]
pub struct ClaimWindowConfig {
    pub resource_name: String,
    pub start_time: u64,
    pub end_time: u64,
    pub required_level: u8,
}

#[derive(Debug, Deserialize)]
pub struct AdminRequest {
    pub resource_name: String,
    pub admin_pubkey: String,
}

#[derive(Debug, Deserialize)]
pub struct AccessRequest {
    pub wallet_address: String,
    pub resource_name: String,
}

pub mod admin {
    use super::*;
    use axum::{Json, extract::State};
    use serde_json::json;

    pub async fn configure_resource(
        State(app_state): State<AppState>,
        Json(config): Json<ResourceConfig>,
    ) -> Result<Json<serde_json::Value>, TokenGatingError> {
        // Configure the resource
        let signature = app_state.token_gating_client
            .configure_resource(config.name.clone(), &config.required_mint)
            .await
            .map_err(|e| {
                tracing::error!("Error configuring resource: {:?}", e);
                TokenGatingError::TransactionError(e.to_string())
            })?;
        
        // Configure levels if provided
        let level_signature = if let Some(levels) = &config.level_thresholds {
            Some(app_state.token_gating_client
                .configure_levels(config.name.clone(), levels.clone())
                .await
                .map_err(|e| {
                    tracing::error!("Error configuring levels: {:?}", e);
                    TokenGatingError::TransactionError(e.to_string())
                })?)
        } else {
            None
        };
        
        // Log the request details
        tracing::info!(
            "Resource configuration request: name={}, mint={}, levels={:?}",
            config.name,
            config.required_mint,
            config.level_thresholds
        );
        
        Ok(Json(json!({
            "success": true,
            "resource_signature": signature,
            "level_signature": level_signature,
            "resource": config.name
        })))
    }
    
    pub async fn configure_claim_window(
        State(app_state): State<AppState>,
        Json(config): Json<ClaimWindowConfig>,
    ) -> Result<Json<serde_json::Value>, TokenGatingError> {
        let signature = app_state.token_gating_client
            .configure_claim(
                config.resource_name.clone(),
                config.start_time,
                config.end_time,
                config.required_level
            )
            .await
            .map_err(|e| {
                tracing::error!("Error configuring claim window: {:?}", e);
                TokenGatingError::TransactionError(e.to_string())
            })?;
        
        tracing::info!(
            "Claim window configuration: resource={}, start={}, end={}, level={}",
            config.resource_name,
            config.start_time,
            config.end_time,
            config.required_level
        );
        
        Ok(Json(json!({
            "success": true,
            "signature": signature,
            "resource": config.resource_name
        })))
    }
    
    pub async fn add_admin(
        State(app_state): State<AppState>,
        Json(request): Json<AdminRequest>,
    ) -> Result<Json<serde_json::Value>, TokenGatingError> {
        // For now, we'll just return a success response
        // In a real implementation, you would call the contract to add the admin
        Ok(Json(json!({
            "status": "success",
            "message": format!("Admin {} added to resource {}", request.admin_pubkey, request.resource_name)
        })))
    }
    
    pub async fn remove_admin(
        State(app_state): State<AppState>,
        Json(request): Json<AdminRequest>,
    ) -> Result<Json<serde_json::Value>, TokenGatingError> {
        // For now, we'll just return a success response
        // In a real implementation, you would call the contract to remove the admin
        Ok(Json(json!({
            "status": "success",
            "message": format!("Admin {} removed from resource {}", request.admin_pubkey, request.resource_name)
        })))
    }
}

pub mod transaction {
    use super::*;
    use axum::{Json, extract::State};
    use serde_json::json;
    use solana_sdk::transaction::Transaction;
    use base64::{Engine as _, engine::general_purpose};

    #[derive(Debug, Deserialize)]
    pub struct TransactionRequest {
        pub serialized_transaction: String,
    }

    pub async fn submit_transaction(
        State(app_state): State<AppState>,
        Json(request): Json<TransactionRequest>,
    ) -> Result<Json<serde_json::Value>, TokenGatingError> {
        // Deserialize the transaction
        let transaction_bytes = general_purpose::STANDARD
            .decode(&request.serialized_transaction)
            .map_err(|e| TokenGatingError::TransactionError(format!("Failed to decode transaction: {}", e)))?;
            
        let transaction: Transaction = bincode::deserialize(&transaction_bytes)
            .map_err(|e| TokenGatingError::TransactionError(format!("Failed to deserialize transaction: {}", e)))?;
            
        // Submit the transaction using the Solana integration
        let signature = app_state.solana_integration.submit_transaction(transaction)
            .map_err(|e| TokenGatingError::TransactionError(format!("Failed to submit transaction: {}", e)))?;
            
        Ok(Json(json!({
            "success": true,
            "signature": signature
        })))
    }
}

pub mod resource {
    use super::*;
    use axum::{Json, extract::{Path, State}};
    use serde_json::json;

    pub async fn get_resource_requirements(
        State(app_state): State<AppState>,
        Path(resource): Path<String>
    ) -> Result<Json<serde_json::Value>, TokenGatingError> {
        // In a real implementation, we would fetch this from the blockchain
        // For now, return mock data
        Ok(Json(json!({
            "resource": resource,
            "required_mint": "TokenMintAddress",
            "level_thresholds": [10, 50, 100]
        })))
    }
} 