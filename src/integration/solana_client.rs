use anchor_client::{
    Client,
    Cluster,
    anchor_lang::system_program,
    solana_sdk::{
        pubkey::Pubkey,
        signature::{Keypair, read_keypair_file, Signature, Signer, SignerError},
        commitment_config::CommitmentConfig,
        transaction::Transaction,
    },
};
use solana_client::rpc_client::RpcClient;
use std::sync::Arc;
use std::str::FromStr;
use std::ops::Deref;
use thiserror::Error;
use crate::errors::TokenGatingError;
use tracing::{info, error};

// Custom wrapper for Keypair that implements Clone
#[derive(Clone)]
pub struct SignerWrapper {
    keypair: Arc<Keypair>,
}

impl SignerWrapper {
    pub fn new() -> Self {
        Self {
            keypair: Arc::new(Keypair::new()),
        }
    }
    
    pub fn from_file(path: &str) -> Result<Self, TokenGatingError> {
        let keypair = read_keypair_file(path)
            .map_err(|e| TokenGatingError::ConfigurationError(format!("Failed to read keypair: {}", e)))?;
        
        Ok(Self {
            keypair: Arc::new(keypair),
        })
    }
}

// Implement Deref for SignerWrapper to allow the blanket Signer implementation to work
impl Deref for SignerWrapper {
    type Target = Keypair;
    fn deref(&self) -> &Self::Target {
        &*self.keypair
    }
}

// Remove explicit Signer implementation to avoid conflict with the blanket implementation

// Custom wrapper for RpcClient that implements Clone
pub struct RpcClientWrapper {
    inner: Arc<RpcClient>,
}

impl RpcClientWrapper {
    pub fn new(url: &str) -> Self {
        Self {
            inner: Arc::new(RpcClient::new(url.to_string())),
        }
    }
}

impl Clone for RpcClientWrapper {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

// Manually implement Debug for RpcClientWrapper
impl std::fmt::Debug for RpcClientWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcClientWrapper")
         .field("inner", &"RpcClient")
         .finish()
    }
}

// Custom wrapper for Anchor Client that can be cloned
pub struct ClientWrapper {
    inner: Arc<Client<SignerWrapper>>,
}

impl ClientWrapper {
    pub fn new(cluster: Cluster, payer: SignerWrapper) -> Self {
        Self {
            inner: Arc::new(Client::new(cluster, payer)),
        }
    }
}

impl Clone for ClientWrapper {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

// Manually implement Debug for ClientWrapper
impl std::fmt::Debug for ClientWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientWrapper")
         .field("inner", &"Client<SignerWrapper>")
         .finish()
    }
}

// RPC client wrapper with Arc for thread safety
pub struct ArcRpcClient {
    pub inner: Arc<RpcClient>,
}

// Implementation of Deref for ArcRpcClient
impl Deref for ArcRpcClient {
    type Target = RpcClient;
    
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// Solana integration with all required clients
pub struct SolanaIntegration {
    pub rpc_client: ArcRpcClient,
}

// Implementation of Solana integration
impl SolanaIntegration {
    pub fn new(rpc_url: &str) -> Result<Self, TokenGatingError> {
        let rpc_client = RpcClient::new(rpc_url.to_string());
        
        // Validate the connection
        match rpc_client.get_version() {
            Ok(_) => {
                info!("Connected to Solana node at {}", rpc_url);
                Ok(Self {
                    rpc_client: ArcRpcClient {
                        inner: Arc::new(rpc_client),
                    }
                })
            },
            Err(e) => {
                error!("Failed to connect to Solana node: {}", e);
                Err(TokenGatingError::ConnectionError(format!("Failed to connect to Solana RPC: {}", e)))
            }
        }
    }
    
    // Helper to create an anchor client
    pub fn create_anchor_client(&self, cluster_name: &str) -> Client<SignerWrapper> {
        let cluster = match cluster_name {
            "mainnet" => Cluster::Mainnet,
            "testnet" => Cluster::Testnet,
            _ => Cluster::Devnet, // Default to devnet
        };
        
        Client::new(
            cluster,
            SignerWrapper::new()
        )
    }

    pub fn submit_transaction(&self, transaction: Transaction) -> Result<String, TokenGatingError> {
        let signature = self.rpc_client.inner.send_and_confirm_transaction(&transaction)
            .map_err(|e| TokenGatingError::TransactionError(e.to_string()))?;
        Ok(signature.to_string())
    }

    pub fn get_token_balance(&self, wallet: &Pubkey, mint: &Pubkey) -> Result<u64, TokenGatingError> {
        let token_account = spl_associated_token_account::get_associated_token_address(wallet, mint);
        let balance = self.rpc_client.inner.get_token_account_balance(&token_account)
            .map_err(|e| TokenGatingError::SolanaRpcError(e.to_string()))?;
        let parsed_balance = balance.amount.parse::<u64>()
            // Option A: Use the new ParsingError variant if added:
            // .map_err(|e| TokenGatingError::ParsingError(format!("Failed to parse balance: {}", e)))?;
            // Option B: Use an existing variant:
            .map_err(|e| TokenGatingError::ConfigurationError(format!("Failed to parse balance: {}", e)))?;
        Ok(parsed_balance)
    }
    
    // Check if an account exists on the blockchain
    pub fn account_exists(&self, address: &Pubkey) -> Result<bool, TokenGatingError> {
        match self.rpc_client.inner.get_account(address) {
            Ok(_) => Ok(true),
            Err(err) => {
                // Check if the error is because the account doesn't exist
                if err.to_string().contains("not found") {
                    Ok(false)
                } else {
                    Err(TokenGatingError::SolanaRpcError(err.to_string()))
                }
            }
        }
    }
    
    // Get an account's data
    pub fn get_account_data(&self, address: &Pubkey) -> Result<Vec<u8>, TokenGatingError> {
        let account = self.rpc_client.inner.get_account(address)
            .map_err(|e| TokenGatingError::SolanaRpcError(e.to_string()))?;
        Ok(account.data)
    }
    
    // Get the SOL balance of an account
    pub fn get_sol_balance(&self, address: &Pubkey) -> Result<u64, TokenGatingError> {
        let balance = self.rpc_client.inner.get_balance(address)
            .map_err(|e| TokenGatingError::SolanaRpcError(e.to_string()))?;
        Ok(balance)
    }
    
    // Get a transaction status
    pub fn get_transaction_status(&self, signature: &str) -> Result<bool, TokenGatingError> {
        let signature = Signature::from_str(signature)
            .map_err(|e| TokenGatingError::ConfigurationError(format!("Invalid signature: {}", e)))?;
            
        match self.rpc_client.inner.get_signature_status(&signature) {
            Ok(Some(status)) => Ok(status.is_ok()),
            Ok(None) => Ok(false), // Transaction not found or not yet processed
            Err(e) => Err(TokenGatingError::SolanaRpcError(e.to_string())),
        }
    }
}
