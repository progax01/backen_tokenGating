use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::json;
use std::sync::Arc;
use tracing::{info, error, debug};

use crate::errors::TokenGatingError;
use crate::integration::solana_client::{SolanaIntegration, SignerWrapper};
use crate::integration::token_gating_client::TokenGatingClient;
use crate::AppState;
use solana_sdk::signature::Keypair;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use anchor_client::Client;

/// Verify user access to a specific resource based on token holdings
///
/// This endpoint checks if a wallet address has sufficient tokens to access a gated resource
pub async fn verify_access(
    State(app_state): State<AppState>,
    Path((address, resource)): Path<(String, String)>
) -> Result<Json<serde_json::Value>, TokenGatingError> {
    info!("Verifying access for wallet {} to resource {}", address, resource);
    
    // Validate wallet address
    let wallet = Pubkey::from_str(&address)
        .map_err(|_| {
            error!("Invalid wallet address: {}", address);
            TokenGatingError::ConfigurationError(format!("Invalid wallet address: {}", address))
        })?;
    
    // Get resource information
    let resource_address = get_resource_address(&resource, &app_state.token_gating_client.program_id())?;
    
    // Check if resource exists
    let resource_exists = app_state.solana_integration
        .account_exists(&resource_address)
        .map_err(|e| {
            error!("Failed to check if resource exists: {}", e);
            TokenGatingError::InternalServerError(e.to_string())
        })?;
    
    if !resource_exists {
        return Err(TokenGatingError::ResourceNotFound);
    }
    
    // In a production implementation, we would fetch the mint address from the resource account
    // For now, use the default SPL token address
    let mint_address = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
    let mint = Pubkey::from_str(mint_address)
        .map_err(|_| TokenGatingError::InvalidResourceConfig)?;
        
    // Get token balance using the solana_integration
    // This will check if the token account exists and get the balance if it does
    let token_balance = match app_state.solana_integration.get_token_balance(&wallet, &mint) {
        Ok(balance) => balance,
        Err(e) => {
            debug!("Failed to get token balance, defaulting to 0: {}", e);
            0  // Default to 0 if token account doesn't exist
        }
    };
    
    // For demo purposes, we're using fixed thresholds
    // In production, you would fetch these from the resource configuration on-chain
    let level_thresholds = vec![10, 50, 100];
    
    // Calculate the user's access level based on token balance
    let current_level = calculate_level(token_balance, &level_thresholds);
    
    // Determine if access is granted (level > 0)
    let access_granted = current_level > 0;
    
    // Set the required level - in production, this would come from the resource config
    let required_level = 1;
    
    // Return a comprehensive response
    Ok(Json(json!({
        "address": address,
        "resource": resource,
        "resource_address": resource_address.to_string(),
        "access_granted": access_granted,
        "has_sufficient_balance": current_level >= required_level,
        "current_level": current_level,
        "token_balance": token_balance,
        "required_level": required_level,
        "level_thresholds": level_thresholds
    })))
}

/// Calculate the user's level based on token balance and thresholds
fn calculate_level(balance: u64, thresholds: &[u64]) -> u8 {
    if thresholds.is_empty() || balance < thresholds[0] {
        return 0;  // No access
    }
    
    let mut level = 1;
    for (index, &threshold) in thresholds.iter().enumerate() {
        if balance >= threshold {
            level = (index + 1) as u8;
        } else {
            break;
        }
    }
    
    level
}

/// Get the resource address from the resource name
fn get_resource_address(resource_name: &str, program_id: &Pubkey) -> Result<Pubkey, TokenGatingError> {
    let resource_seeds = [resource_name.as_bytes()];
    let (resource_pubkey, _) = Pubkey::find_program_address(&resource_seeds, program_id);
    Ok(resource_pubkey)
} 