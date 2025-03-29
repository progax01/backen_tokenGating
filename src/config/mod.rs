use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SolanaConfig {
    pub rpc_url: String,
    pub cluster: String,
    pub program_id: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub server: ServerConfig,
    pub solana: SolanaConfig,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        // Determine the environment
        let env = env::var("RUN_ENV").unwrap_or_else(|_| "development".into());

        let config = Config::builder()
            // Default configuration for server
            .set_default("server.port", 8090)?
            
            // Default configuration for Solana
            .set_default("solana.cluster", "devnet")?
            .set_default("solana.rpc_url", "http://localhost:8899")?
            .set_default("solana.program_id", "EHyipzMTK3FV63inZTyvqmSYqFxDdKJPKMhHay6yQ9mw")?  // Example program ID
            
            // Add configuration file based on environment
            .add_source(File::with_name(&format!("config/{}", env)).required(false))
            
            // Add environment variable overrides
            .add_source(Environment::with_prefix("TOKEN_GATE").separator("_"))
            
            .build()?;

        // Try to deserialize and log values for debugging
        let settings: Settings = config.try_deserialize()?;
        
        // Print loaded configuration (be careful with sensitive data in production)
        println!("Loaded configuration:");
        println!("Server port: {}", settings.server.port);
        println!("Solana cluster: {}", settings.solana.cluster);
        println!("Solana RPC URL: {}", settings.solana.rpc_url);
        println!("Solana program ID: {}", settings.solana.program_id);
        
        Ok(settings)
    }
} 