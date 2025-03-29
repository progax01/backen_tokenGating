mod api;
mod business_logic;
mod config;
mod errors;
mod integration;

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::path::Path;
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::TraceLayer;
use tracing_subscriber;
use dotenv::dotenv;
use tokio::signal;
use tracing::{info, warn, error, Level};
use clap::Parser;

use integration::token_gating_client::TokenGatingClient;
use integration::solana_client::{SolanaIntegration, SignerWrapper};
use solana_sdk::signature::Keypair;
use solana_sdk::signature::read_keypair_file;
use solana_sdk::signer::Signer;

// Health check endpoint - moved from bottom of file
async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port number for the server
    #[arg(short, long, default_value_t = 8090)]
    port: u16,
}

// Define a shared state for our application
#[derive(Clone)]
pub struct AppState {
    pub solana_integration: Arc<SolanaIntegration>,
    pub token_gating_client: Arc<TokenGatingClient>,
}

// Define configuration struct for our application
#[derive(Debug, Clone)]
pub struct Config {
    pub server_port: u16,
    pub solana_cluster: String,
    pub solana_rpc_url: String,
    pub program_id: String,
    pub admin_keypair_path: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        Ok(Self {
            server_port: 8090,
            solana_cluster: std::env::var("SOLANA_CLUSTER").unwrap_or_else(|_| "devnet".to_string()),
            solana_rpc_url: std::env::var("SOLANA_RPC_URL").unwrap_or_else(|_| 
                "https://api.devnet.solana.com".to_string()),
            program_id: std::env::var("PROGRAM_ID").unwrap_or_else(|_| 
                "EHyipzMTK3FV63inZTyvqmSYqFxDdKJPKMhHay6yQ9mw".to_string()),
            admin_keypair_path: std::env::var("ADMIN_KEYPAIR_PATH").ok(),
        })
    }

    pub fn default() -> Self {
        Self {
            server_port: 8090,
            solana_cluster: "devnet".to_string(),
            solana_rpc_url: "https://api.devnet.solana.com".to_string(),
            program_id: "EHyipzMTK3FV63inZTyvqmSYqFxDdKJPKMhHay6yQ9mw".to_string(),
            admin_keypair_path: None,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args = Args::parse();
    
    // Load .env file if present
    dotenv().ok();
    
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    // Load configuration
    let config = Config::load().unwrap_or_else(|e| {
        warn!("Failed to load config, using defaults: {}", e);
        Config::default()
    });
    
    println!("Loaded configuration:");
    println!("Server port: {}", config.server_port);
    println!("Solana cluster: {}", config.solana_cluster);
    println!("Solana RPC URL: {}", config.solana_rpc_url);
    println!("Solana program ID: {}", config.program_id);

    // Create Solana integration (for low-level operations)
    let solana_integration = Arc::new(
        SolanaIntegration::new(&config.solana_rpc_url)?
    );


    // Create the main TokenGatingClient (higher-level business logic)
    let admin_keypair = if let Some(path) = config.admin_keypair_path {
        match read_keypair_file(&path) {
            Ok(keypair) => {
                info!("Using admin keypair from file: {} (public key: {})", path, keypair.pubkey());
                keypair
            }
            Err(e) => {
                warn!("Failed to load admin keypair from file: {}", e);
                warn!("Using randomly generated keypair with public key: {}", Keypair::new().pubkey());
                warn!("IMPORTANT: This keypair has no funds. Fund it or set ADMIN_KEYPAIR_PATH env variable.");
                Keypair::new()
            }
        }
    } else {
        // Try to use the default Solana keypair location
        let home_dir = dirs::home_dir().unwrap_or_default();
        let default_keypair_path = home_dir.join(".config/solana/id.json");
        println!("Default keypair path: {}", default_keypair_path.to_str().unwrap());
        if default_keypair_path.exists() {
            match read_keypair_file(default_keypair_path.to_str().unwrap()) {
                Ok(keypair) => {
                    info!("Using default Solana keypair from ~/.config/solana/id.json (public key: {})", 
                          keypair.pubkey());
                    keypair
                }
                Err(e) => {
                    warn!("Failed to load default Solana keypair: {}", e);
                    let keypair = Keypair::new();
                    warn!("Using randomly generated keypair with public key: {}", keypair.pubkey());
                    warn!("IMPORTANT: This keypair has no funds. You need to add SOL to this address.");
                    keypair
                }
            }
        } else {
            let keypair = Keypair::new();
            warn!("No keypair file found at ~/.config/solana/id.json");
            warn!("Using randomly generated keypair with public key: {}", keypair.pubkey());
            warn!("IMPORTANT: This keypair has no funds. You need to add SOL to this address.");
            keypair
        }
    };
    
    // Create anchor client for TokenGatingClient
    let cluster = match config.solana_cluster.as_str() {
        "mainnet" => anchor_client::Cluster::Mainnet,
        "testnet" => anchor_client::Cluster::Testnet,
        _ => anchor_client::Cluster::Devnet, // Default to devnet
    };
    
    let anchor_client = anchor_client::Client::new(
        cluster,
        SignerWrapper::new()
    );
    
    // Create TokenGatingClient from the cluster settings and program ID
    let token_gating_client = Arc::new(
        TokenGatingClient::new(
            anchor_client,
            admin_keypair, 
            &config.program_id
        )?
    );

    // Create combined app state
    let app_state = AppState {
        solana_integration: solana_integration.clone(),
        token_gating_client: token_gating_client.clone(),
    };

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Define routes
    let app = Router::new()
        // Admin routes
        .route("/admin/configure-resource", post(api::handlers::admin::configure_resource))
        .route("/admin/configure-claim", post(api::handlers::admin::configure_claim))
        .route("/admin/add", post(api::handlers::admin::add_admin))
        .route("/admin/remove", post(api::admin::remove_admin))
        
        // Access routes
        .route("/access/verify/:wallet/:resource", get(api::handlers::access::verify_access))
        
        // Transaction routes
        .route("/transaction/submit", post(api::transaction::submit_transaction))
        
        // Resource routes
        .route("/resource/requirements/:resource", get(api::resource::get_resource_requirements))
        
        // Health check route
        .route("/health", get(health_check))
        
        // Apply middleware and state
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state.clone());

    // Server address
    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));

    info!("Solana Token Gating backend starting on {}", addr);
    info!("Connected to Solana cluster: {}", config.solana_cluster);
    info!("Using program ID: {}", config.program_id);

    // Start server with graceful shutdown
    let server = axum::Server::bind(&addr)
        .serve(app.into_make_service());
    
    // Add graceful shutdown
    let graceful = server.with_graceful_shutdown(shutdown_signal(app_state));
    
    // Start the server
    if let Err(err) = graceful.await {
        error!("Server error: {}", err);
    }
    
    Ok(())
}

// Shutdown signal handler
async fn shutdown_signal(app_state: AppState) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, starting graceful shutdown");
    
    // Perform any cleanup needed
    if let Err(err) = app_state.token_gating_client.shutdown().await {
        warn!("Error during TokenGatingClient shutdown: {}", err);
    }
    
    info!("Graceful shutdown completed");
}