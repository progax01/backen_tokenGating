use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;

use crate::errors::TokenGatingError;
use crate::integration::solana_client::SolanaIntegration;

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceConfig {
    pub name: String,
    pub required_mint: String,
    pub level_thresholds: Vec<u64>,
    pub claim_window_start: Option<u64>,
    pub claim_window_end: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessResult {
    pub access_granted: bool,
    pub current_level: u8,
    pub required_level: u8,
}

#[derive(Clone)]
pub struct ResourceAccessManager {
    solana_integration: Arc<SolanaIntegration>,
}

impl ResourceAccessManager {
    pub fn new(solana_integration: SolanaIntegration) -> Self {
        Self { 
            solana_integration: Arc::new(solana_integration) 
        }
    }

    pub fn verify_access(
        &self, 
        wallet_address: &str, 
        resource_name: &str
    ) -> Result<AccessResult, TokenGatingError> {
        // Parse wallet and mint addresses
        let wallet = Pubkey::from_str(wallet_address)
            .map_err(|_| TokenGatingError::InvalidResourceConfig)?;

        // Retrieve resource configuration (in a real implementation, this would come from on-chain data)
        let resource_config = self.get_resource_config(resource_name)?;

        // Parse mint address
        let mint = Pubkey::from_str(&resource_config.required_mint)
            .map_err(|_| TokenGatingError::InvalidResourceConfig)?;

        // Get token balance
        let token_balance = self.solana_integration.get_token_balance(&wallet, &mint)?;

        // Determine access level based on token balance
        let current_level = self.calculate_access_level(token_balance, &resource_config.level_thresholds);
        
        // Check claim window if applicable
        self.validate_claim_window(&resource_config)?;

        Ok(AccessResult {
            access_granted: current_level > 0,
            current_level,
            required_level: 1, // Minimum level for access
        })
    }

    fn calculate_access_level(&self, balance: u64, thresholds: &[u64]) -> u8 {
        // Determine access level based on token balance
        for (index, &threshold) in thresholds.iter().enumerate() {
            if balance >= threshold {
                return (index + 1) as u8;
            }
        }
        0 // No access
    }

    fn validate_claim_window(&self, config: &ResourceConfig) -> Result<(), TokenGatingError> {
        // Check if claim window is active
        let current_timestamp = chrono::Utc::now().timestamp() as u64;

        match (config.claim_window_start, config.claim_window_end) {
            (Some(start), Some(end)) => {
                if current_timestamp < start || current_timestamp > end {
                    return Err(TokenGatingError::ClaimWindowClosed);
                }
            },
            _ => {} // No claim window restrictions
        }

        Ok(())
    }

    fn get_resource_config(&self, resource_name: &str) -> Result<ResourceConfig, TokenGatingError> {
        // In a real implementation, this would fetch from on-chain data
        // This is a mock implementation for demonstration
        Ok(ResourceConfig {
            name: resource_name.to_string(),
            required_mint: "TokenMintAddress".to_string(),
            level_thresholds: vec![10, 50, 100],
            claim_window_start: Some(chrono::Utc::now().timestamp() as u64),
            claim_window_end: Some(chrono::Utc::now().timestamp() as u64 + 86400), // 24 hours from now
        })
    }
} 