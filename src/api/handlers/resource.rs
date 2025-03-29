use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::errors::TokenGatingError;
use crate::AppState;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tracing::{info, error};

pub async fn get_resource_requirements(
    State(app_state): State<AppState>,
    Path(resource): Path<String>
) -> Result<Json<serde_json::Value>, TokenGatingError> {
    info!("Fetching resource requirements for {}", resource);
    
    // This would normally fetch the resource configuration from the blockchain
    // For a production implementation, you would:
    // 1. Get the resource PDA
    // 2. Fetch the account data
    // 3. Deserialize the account data to get the resource configuration
    
    let resource_address = get_resource_address(&resource, &app_state.token_gating_client.program_id())?;
    
    // Fetch token mint for the resource - in production, get this from the blockchain
    let mint_address = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"; // Default SPL token program for example
    
    // Check if the account exists on chain
    let account_exists = app_state.solana_integration
        .account_exists(&resource_address)
        .map_err(|e| {
            error!("Failed to check if resource account exists: {}", e);
            TokenGatingError::InternalServerError(e.to_string())
        })?;
    
    if !account_exists {
        return Err(TokenGatingError::ResourceNotFound);
    }
    
    // For now, return simulated data
    // In production, deserialize the account data to get the proper thresholds
    let level_thresholds = vec![10, 50, 100];
    
    Ok(Json(json!({
        "resource": resource,
        "resource_address": resource_address.to_string(),
        "required_mint": mint_address,
        "level_thresholds": level_thresholds,
        "is_active": true
    })))
}

// Helper function to derive the resource PDA
fn get_resource_address(resource_name: &str, program_id: &Pubkey) -> Result<Pubkey, TokenGatingError> {
    let resource_seeds = [resource_name.as_bytes()];
    let (resource_pubkey, _) = Pubkey::find_program_address(&resource_seeds, program_id);
    Ok(resource_pubkey)
} 