use anchor_client::{
    Client,
    Program,
    anchor_lang::system_program,
    solana_sdk::{
        pubkey::Pubkey,
        signature::{Keypair, Signature},
        instruction::{Instruction, AccountMeta},
        commitment_config::CommitmentConfig,
    },
};
use std::sync::Arc;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use solana_sdk::signature::Signer;
use anchor_client::{
    anchor_lang::{ToAccountMetas, InstructionData},
};
use crate::errors::TokenGatingError;
use crate::integration::solana_client::SignerWrapper;
use tokio::sync::{mpsc, oneshot};
use std::thread;
use tracing::{info, error, debug, warn};
use std::time::Duration;

// Define program instructions and accounts
mod token_gating {
    use anchor_client::solana_sdk::pubkey::Pubkey;
    use serde::{Deserialize, Serialize};
    use anchor_client::anchor_lang::{ToAccountMetas, InstructionData, AnchorSerialize};
    use solana_sdk::instruction::AccountMeta;

    pub mod accounts {
        use super::*;

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct ConfigureResource {
            pub admin: Pubkey,
            pub resource: Pubkey,
            pub system_program: Pubkey,
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct VerifyAccess {
            pub user: Pubkey,
            pub resource: Pubkey,
            pub user_token_account: Pubkey,
            pub token_program: Pubkey,
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct ConfigureLevels {
            pub admin: Pubkey,
            pub resource: Pubkey,
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct ConfigureClaim {
            pub admin: Pubkey,
            pub resource: Pubkey,
            pub claim_window: Pubkey,
            pub system_program: Pubkey,
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct ClaimToken {
            pub user: Pubkey,
            pub resource: Pubkey,
            pub claim_window: Pubkey,
            pub user_claim_record: Pubkey,
            pub system_program: Pubkey,
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct AddAdmin {
            pub admin: Pubkey,
            pub resource: Pubkey,
            pub new_admin: Pubkey,
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct RemoveAdmin {
            pub admin: Pubkey,
            pub resource: Pubkey,
            pub admin_to_remove: Pubkey,
        }

        // Implement ToAccountMetas for all account structs
        impl ToAccountMetas for ConfigureResource {
            fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
                vec![
                    AccountMeta::new_readonly(self.admin, true),
                    AccountMeta::new(self.resource, false),
                    AccountMeta::new_readonly(self.system_program, false),
                ]
            }
        }

        impl ToAccountMetas for VerifyAccess {
            fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
                vec![
                    AccountMeta::new_readonly(self.user, true),
                    AccountMeta::new_readonly(self.resource, false),
                    AccountMeta::new_readonly(self.user_token_account, false),
                    AccountMeta::new_readonly(self.token_program, false),
                ]
            }
        }

        impl ToAccountMetas for ConfigureLevels {
            fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
                vec![
                    AccountMeta::new_readonly(self.admin, true),
                    AccountMeta::new(self.resource, false),
                ]
            }
        }

        impl ToAccountMetas for ConfigureClaim {
            fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
                vec![
                    AccountMeta::new_readonly(self.admin, true),
                    AccountMeta::new_readonly(self.resource, false),
                    AccountMeta::new(self.claim_window, false),
                    AccountMeta::new_readonly(self.system_program, false),
                ]
            }
        }

        impl ToAccountMetas for ClaimToken {
            fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
                vec![
                    AccountMeta::new_readonly(self.user, true),
                    AccountMeta::new_readonly(self.resource, false),
                    AccountMeta::new_readonly(self.claim_window, false),
                    AccountMeta::new(self.user_claim_record, false),
                    AccountMeta::new_readonly(self.system_program, false),
                ]
            }
        }

        impl ToAccountMetas for AddAdmin {
            fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
                vec![
                    AccountMeta::new_readonly(self.admin, true),
                    AccountMeta::new(self.resource, false),
                    AccountMeta::new_readonly(self.new_admin, false),
                ]
            }
        }

        impl ToAccountMetas for RemoveAdmin {
            fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
                vec![
                    AccountMeta::new_readonly(self.admin, true),
                    AccountMeta::new(self.resource, false),
                    AccountMeta::new_readonly(self.admin_to_remove, false),
                ]
            }
        }
    }

    pub mod instruction {
        use super::*;

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct ConfigureResource {
            pub name: String,
            pub required_mint: Pubkey,
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct VerifyAccess {
            pub resource_name: String,
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct ConfigureLevels {
            pub level_thresholds: Vec<u64>,
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct ConfigureClaim {
            pub start_time: u64,
            pub end_time: u64,
            pub required_level: u8,
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct ClaimToken {
            pub resource_name: String,
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct AddAdmin {
            pub resource_name: String,
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct RemoveAdmin {
            pub resource_name: String,
        }

        // Instead of implementing InstructionData trait which requires AnchorSerialize and Discriminator
        // we'll define our own instruction creation methods
    }
}

// Define custom spl token program ID
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const MAX_TRANSACTION_RETRIES: u8 = 3;
const RETRY_DELAY_MS: u64 = 1000;

// Define custom error type
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Transaction error: {0}")]
    TransactionError(String),
    
    #[error("Program error: {0}")]
    ProgramError(String),
    
    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),
    
    #[error("Invalid resource configuration: {0}")]
    InvalidResourceConfig(String),
    
    #[error("Access denied")]
    AccessDenied,
    
    #[error("Claim window closed")]
    ClaimWindowClosed,
    
    #[error("Channel error: {0}")]
    ChannelError(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl From<anchor_client::ClientError> for ClientError {
    fn from(err: anchor_client::ClientError) -> Self {
        ClientError::TransactionError(err.to_string())
    }
}

impl From<ClientError> for TokenGatingError {
    fn from(err: ClientError) -> Self {
        match err {
            ClientError::TransactionError(msg) => TokenGatingError::TransactionError(msg),
            ClientError::InvalidPublicKey(msg) => TokenGatingError::ConfigurationError(msg),
            ClientError::InvalidResourceConfig(_) => TokenGatingError::InvalidResourceConfig,
            ClientError::AccessDenied => TokenGatingError::AccessDenied,
            ClientError::ClaimWindowClosed => TokenGatingError::ClaimWindowClosed,
            ClientError::ChannelError(msg) => TokenGatingError::InternalServerError(msg),
            ClientError::InternalError(msg) => TokenGatingError::InternalServerError(msg),
            ClientError::ProgramError(msg) => TokenGatingError::InternalServerError(msg),
        }
    }
}

// Transaction commands for the background processor
enum TransactionCommand {
    ConfigureResource {
        admin_keypair: Arc<Keypair>,
        program_id: Pubkey,
        name: String,
        required_mint: Pubkey,
        response_tx: oneshot::Sender<Result<Signature, ClientError>>,
    },
    ConfigureLevels {
        admin_keypair: Arc<Keypair>,
        program_id: Pubkey,
        resource_pubkey: Pubkey,
        level_thresholds: Vec<u64>,
        response_tx: oneshot::Sender<Result<Signature, ClientError>>,
    },
    ConfigureClaim {
        admin_keypair: Arc<Keypair>,
        program_id: Pubkey,
        resource_pubkey: Pubkey,
        claim_window_pubkey: Pubkey,
        start_time: u64,
        end_time: u64,
        required_level: u8,
        response_tx: oneshot::Sender<Result<Signature, ClientError>>,
    },
    AddAdmin {
        admin_keypair: Arc<Keypair>,
        program_id: Pubkey,
        resource_pubkey: Pubkey,
        new_admin: Pubkey,
        resource_name: String,
        response_tx: oneshot::Sender<Result<Signature, ClientError>>,
    },
    RemoveAdmin {
        admin_keypair: Arc<Keypair>,
        program_id: Pubkey,
        resource_pubkey: Pubkey,
        admin_to_remove: Pubkey,
        resource_name: String,
        response_tx: oneshot::Sender<Result<Signature, ClientError>>,
    },
    GetTokenBalance {
        wallet: Pubkey,
        mint: Pubkey,
        response_tx: oneshot::Sender<Result<u64, ClientError>>,
    },
    Shutdown,
}

pub struct TokenGatingClient {
    program: Program<SignerWrapper>,
    admin_keypair: Arc<Keypair>,
    program_id: Pubkey,
    tx_sender: mpsc::Sender<TransactionCommand>,
}

impl TokenGatingClient {
    pub fn new(client: Client<SignerWrapper>, admin_keypair: Keypair, program_id: &str) -> Result<Self, ClientError> {
        let program_id = Pubkey::from_str(program_id)
            .map_err(|_| ClientError::InvalidPublicKey("Invalid program ID".to_string()))?;
        
        let admin = Arc::new(admin_keypair);
        let program = client.program(program_id)?;
        
        // Create a channel for transaction commands
        let (tx_sender, tx_receiver) = mpsc::channel(100);
        
        // Spawn a background thread to process transactions
        let worker_program_id = program_id;
        let client_url = program.rpc().url().to_string();
        let worker_admin = admin.clone();
        
        thread::spawn(move || {
            // Run a separate runtime for handling Solana transactions
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create transaction runtime");
            
            runtime.block_on(async {
                Self::transaction_processor(tx_receiver, client_url, worker_program_id, worker_admin).await;
            });
        });
        
        Ok(Self {
            program,
            admin_keypair: admin,
            program_id,
            tx_sender,
        })
    }
    
    // Getter for program_id
    pub fn program_id(&self) -> Pubkey {
        self.program_id
    }
    
    // Transaction processor that runs in a dedicated thread/runtime
    async fn transaction_processor(
        mut rx: mpsc::Receiver<TransactionCommand>,
        client_url: String,
        program_id: Pubkey,
        _admin_keypair: Arc<Keypair>,
    ) {
        info!("Transaction processor started");
        
        // Process commands from the channel
        while let Some(cmd) = rx.recv().await {
            if matches!(cmd, TransactionCommand::Shutdown) {
                info!("Transaction processor shutting down");
                break;
            }
            
            match cmd {
                TransactionCommand::ConfigureResource { 
                    admin_keypair, 
                    program_id, 
                    name, 
                    required_mint, 
                    response_tx 
                } => {
                    let name_clone = name.clone();
                    let admin_clone = admin_keypair.clone();
                    let client_url_clone = client_url.clone();
                    
                    // Spawn a thread for this specific request
                    std::thread::spawn(move || {
                        // Create a new connection for each request to avoid runtime conflicts
                        let payer = SignerWrapper::new();
                        let client = match anchor_client::Client::new_with_options(
                            anchor_client::Cluster::Custom(client_url_clone.clone(), client_url_clone),
                            payer,
                            CommitmentConfig::confirmed()
                        ).program(program_id) {
                            Ok(program) => program,
                            Err(e) => {
                                error!("Failed to create program in transaction processor: {}", e);
                                let _ = response_tx.send(Err(ClientError::ProgramError(e.to_string())));
                                return;
                            }
                        };
                        
                        debug!("Processing ConfigureResource command for {}", name);
                        
                        // Generate PDA for the resource account
                        let resource_seeds = &[name.as_bytes()];
                        let (resource_pubkey, _) = Pubkey::find_program_address(resource_seeds, &program_id);
                        debug!("Resource PDA: {}", resource_pubkey);
                        
                        // Check admin wallet balance
                        match client.rpc().get_balance(&admin_clone.pubkey()) {
                            Ok(balance) => {
                                info!("Admin wallet balance: {} SOL", balance as f64 / 1_000_000_000.0);
                            },
                            Err(e) => {
                                error!("Failed to check admin balance: {}", e);
                            }
                        }
                        
                        // Anchor instructions use 8-byte discriminators 
                        // This is generated from the anchor IDL
                        // For simplicity, we can use configureResource's discriminator
                        let discriminator: [u8; 8] = [165, 121, 139, 89, 114, 167, 122, 146];
                        
                        // Create the instruction data
                        let mut data = Vec::with_capacity(100);
                        data.extend_from_slice(&discriminator);
                        
                        // Serialize the name as a string (u32 length + bytes)
                        let name_bytes = name.as_bytes();
                        let name_len = name_bytes.len() as u32;
                        data.extend_from_slice(&name_len.to_le_bytes());
                        data.extend_from_slice(name_bytes);
                        
                        // Serialize the required_mint pubkey
                        data.extend_from_slice(&required_mint.to_bytes());
                        
                        info!("Admin: {}, Resource: {}, Required Mint: {}", 
                            admin_clone.pubkey(), resource_pubkey, required_mint);
                        
                        // Build the instruction manually with the correct account structure
                        // According to the IDL: admin, resource, systemProgram
                        let accounts = vec![
                            AccountMeta::new(admin_clone.pubkey(), true),  // Admin is signer and writable
                            AccountMeta::new(resource_pubkey, false),      // Resource PDA is writable but not a signer
                            AccountMeta::new_readonly(system_program::ID, false), // System program
                        ];
                        
                        let ix = Instruction {
                            program_id,
                            accounts,
                            data,
                        };
                        
                        info!("Sending transaction with instruction data: {:?}", ix);
                        
                        // Build and send the transaction
                        match client.request()
                            .instruction(ix)
                            .signer(admin_clone.as_ref())
                            .send() {
                            Ok(signature) => {
                                info!("Resource {} configured successfully: {}", name, signature);
                                let _ = response_tx.send(Ok(signature));
                            },
                            Err(e) => {
                                error!("Failed to configure resource: {}", e);
                                let _ = response_tx.send(Err(ClientError::TransactionError(e.to_string())));
                            }
                        };
                    });
                },
                TransactionCommand::GetTokenBalance { 
                    wallet, 
                    mint, 
                    response_tx 
                } => {
                    let client_url_clone = client_url.clone();
                    
                    std::thread::spawn(move || {
                        // Create a new client for each request
                        let payer = SignerWrapper::new();
                        let client = match anchor_client::Client::new_with_options(
                            anchor_client::Cluster::Custom(client_url_clone.clone(), client_url_clone),
                            payer,
                            CommitmentConfig::confirmed()
                        ).program(program_id) {
                            Ok(program) => program,
                            Err(e) => {
                                error!("Failed to create program in transaction processor: {}", e);
                                let _ = response_tx.send(Err(ClientError::ProgramError(e.to_string())));
                                return;
                            }
                        };
                        
                        debug!("Processing GetTokenBalance command for wallet {}", wallet);
                        
                        // Get the associated token account
                        let token_account = spl_associated_token_account::get_associated_token_address(
                            &wallet,
                            &mint
                        );
                        
                        // Query the token account balance
                        match client.rpc().get_token_account_balance(&token_account) {
                            Ok(balance) => {
                                info!("Token balance for wallet {}: {}", wallet, balance.ui_amount.unwrap_or(0.0));
                                let _ = response_tx.send(Ok(balance.amount.parse::<u64>().unwrap_or(0)));
                            },
                            Err(err) => {
                                warn!("Failed to get token balance: {}", err);
                                let _ = response_tx.send(Err(ClientError::TransactionError(format!("Failed to get token balance: {}", err))));
                            }
                        };
                    });
                },
                TransactionCommand::Shutdown => {
                    info!("Transaction processor shutting down");
                    break;
                },
                _ => {
                    warn!("Unhandled transaction type");
                }
            }
        }
        
        info!("Transaction processor terminated");
    }
    
    // Helper method to execute a transaction with retries
    async fn execute_with_retry<F>(mut transaction_fn: F, max_retries: u8) -> Result<Signature, ClientError>
    where
        F: FnMut() -> Result<Signature, anchor_client::ClientError>,
    {
        let mut attempts = 0;
        let mut last_error = None;
        
        while attempts < max_retries {
            match transaction_fn() {
                Ok(signature) => {
                    return Ok(signature);
                }
                Err(e) => {
                    attempts += 1;
                    last_error = Some(e);
                    
                    if attempts < max_retries {
                        warn!("Transaction attempt {} failed, retrying in {}ms: {}", 
                              attempts, RETRY_DELAY_MS, last_error.as_ref().unwrap());
                        tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                    }
                }
            }
        }
        
        Err(ClientError::TransactionError(format!(
            "Transaction failed after {} attempts: {}",
            max_retries,
            last_error.unwrap()
        )))
    }
    
    // Configure a new resource
    pub async fn configure_resource(&self, name: String, required_mint: &str) -> Result<String, ClientError> {
        let mint_pubkey = Pubkey::from_str(required_mint)
            .map_err(|_| ClientError::InvalidPublicKey(format!("Invalid mint address: {}", required_mint)))?;
        
        // Create a oneshot channel for the response
        let (response_tx, response_rx) = oneshot::channel();
        
        // Send the command to the transaction processor
        self.tx_sender
            .send(TransactionCommand::ConfigureResource {
                admin_keypair: self.admin_keypair.clone(),
                program_id: self.program_id,
                name: name.clone(),
                required_mint: mint_pubkey,
                response_tx,
            })
            .await
            .map_err(|e| ClientError::ChannelError(format!("Failed to send transaction command: {}", e)))?;
        
        // Wait for the response
        let signature = response_rx
            .await
            .map_err(|e| ClientError::ChannelError(format!("Failed to receive transaction result: {}", e)))?
            .map(|sig| sig.to_string())?;
        
        Ok(signature)
    }
    
    // Verify user access
    pub async fn verify_access(&self, resource_name: String, user_pubkey: &str, mint_pubkey: &str) -> Result<bool, ClientError> {
        let user_pubkey = Pubkey::from_str(user_pubkey)
            .map_err(|_| ClientError::InvalidPublicKey(format!("Invalid user public key: {}", user_pubkey)))?;
        
        let mint_pubkey = Pubkey::from_str(mint_pubkey)
            .map_err(|_| ClientError::InvalidPublicKey(format!("Invalid mint address: {}", mint_pubkey)))?;
        
        // Get token balance
        let balance = self.get_token_balance(user_pubkey.to_string().as_str(), mint_pubkey.to_string().as_str()).await?;
        
        // Get resource configuration (thresholds)
        let resource_seeds = [resource_name.as_bytes()];
        let (resource_pubkey, _) = Pubkey::find_program_address(&resource_seeds, &self.program_id);
        
        // This would normally fetch the resource config from the blockchain
        // For now, we'll use default thresholds
        let thresholds = vec![10, 50, 100];
        
        // Determine if the user has access
        let has_access = balance >= thresholds[0];
        
        if has_access {
            info!("Access granted for user {} to resource {}", user_pubkey, resource_name);
        } else {
            info!("Access denied for user {} to resource {}", user_pubkey, resource_name);
        }
        
        Ok(has_access)
    }
    
    // Configure level thresholds
    pub async fn configure_levels(&self, resource_name: String, level_thresholds: Vec<u64>) -> Result<String, ClientError> {
        if level_thresholds.len() > 3 {
            return Err(ClientError::InvalidResourceConfig("Maximum 3 level thresholds allowed".to_string()));
        }
        
        let resource_seeds = [resource_name.as_bytes()];
        let (resource_pubkey, _) = Pubkey::find_program_address(&resource_seeds, &self.program_id);
        
        // Create a oneshot channel for the response
        let (response_tx, response_rx) = oneshot::channel();
        
        // Send the command to the transaction processor
        self.tx_sender
            .send(TransactionCommand::ConfigureLevels {
                admin_keypair: self.admin_keypair.clone(),
                program_id: self.program_id,
                resource_pubkey,
                level_thresholds,
                response_tx,
            })
            .await
            .map_err(|e| ClientError::ChannelError(format!("Failed to send transaction command: {}", e)))?;
        
        // Wait for the response
        let signature = response_rx
            .await
            .map_err(|e| ClientError::ChannelError(format!("Failed to receive transaction result: {}", e)))?
            .map(|sig| sig.to_string())?;
        
        Ok(signature)
    }
    
    // Configure claim window
    pub async fn configure_claim(&self, resource_name: String, start_time: u64, end_time: u64, required_level: u8) -> Result<String, ClientError> {
        if required_level > 3 {
            return Err(ClientError::InvalidResourceConfig("Maximum level is 3".to_string()));
        }
        
        let resource_seeds = [resource_name.as_bytes()];
        let (resource_pubkey, _) = Pubkey::find_program_address(&resource_seeds, &self.program_id);
        
        let claim_window_seeds = [b"claim", resource_name.as_bytes()];
        let (claim_window_pubkey, _) = Pubkey::find_program_address(&claim_window_seeds, &self.program_id);
        
        // Create a oneshot channel for the response
        let (response_tx, response_rx) = oneshot::channel();
        
        // Send the command to the transaction processor
        self.tx_sender
            .send(TransactionCommand::ConfigureClaim {
                admin_keypair: self.admin_keypair.clone(),
                program_id: self.program_id,
                resource_pubkey,
                claim_window_pubkey,
                start_time,
                end_time,
                required_level,
                response_tx,
            })
            .await
            .map_err(|e| ClientError::ChannelError(format!("Failed to send transaction command: {}", e)))?;
        
        // Wait for the response
        let signature = response_rx
            .await
            .map_err(|e| ClientError::ChannelError(format!("Failed to receive transaction result: {}", e)))?
            .map(|sig| sig.to_string())?;
        
        Ok(signature)
    }
    
    // Add admin
    pub async fn add_admin(&self, resource_name: String, new_admin_pubkey: &str) -> Result<String, ClientError> {
        let new_admin = Pubkey::from_str(new_admin_pubkey)
            .map_err(|_| ClientError::InvalidPublicKey(format!("Invalid admin public key: {}", new_admin_pubkey)))?;
        
        let resource_seeds = [resource_name.as_bytes()];
        let (resource_pubkey, _) = Pubkey::find_program_address(&resource_seeds, &self.program_id);
        
        // Create a oneshot channel for the response
        let (response_tx, response_rx) = oneshot::channel();
        
        // Send the command to the transaction processor
        self.tx_sender
            .send(TransactionCommand::AddAdmin {
                admin_keypair: self.admin_keypair.clone(),
                program_id: self.program_id,
                resource_pubkey,
                    new_admin,
                resource_name: resource_name.clone(),
                response_tx,
            })
            .await
            .map_err(|e| ClientError::ChannelError(format!("Failed to send transaction command: {}", e)))?;
        
        // Wait for the response
        let signature = response_rx
            .await
            .map_err(|e| ClientError::ChannelError(format!("Failed to receive transaction result: {}", e)))?
            .map(|sig| sig.to_string())?;
        
        Ok(signature)
    }
    
    // Remove admin
    pub async fn remove_admin(&self, resource_name: String, admin_to_remove_pubkey: &str) -> Result<String, ClientError> {
        let admin_to_remove = Pubkey::from_str(admin_to_remove_pubkey)
            .map_err(|_| ClientError::InvalidPublicKey(format!("Invalid admin public key: {}", admin_to_remove_pubkey)))?;
        
        let resource_seeds = [resource_name.as_bytes()];
        let (resource_pubkey, _) = Pubkey::find_program_address(&resource_seeds, &self.program_id);
        
        // Create a oneshot channel for the response
        let (response_tx, response_rx) = oneshot::channel();
        
        // Send the command to the transaction processor
        self.tx_sender
            .send(TransactionCommand::RemoveAdmin {
                admin_keypair: self.admin_keypair.clone(),
                program_id: self.program_id,
                resource_pubkey,
                    admin_to_remove,
                resource_name: resource_name.clone(),
                response_tx,
            })
            .await
            .map_err(|e| ClientError::ChannelError(format!("Failed to send transaction command: {}", e)))?;
        
        // Wait for the response
        let signature = response_rx
            .await
            .map_err(|e| ClientError::ChannelError(format!("Failed to receive transaction result: {}", e)))?
            .map(|sig| sig.to_string())?;
        
        Ok(signature)
    }
    
    // Get token balance
    pub async fn get_token_balance(&self, wallet_pubkey: &str, mint_pubkey: &str) -> Result<u64, ClientError> {
        let wallet = Pubkey::from_str(wallet_pubkey)
            .map_err(|_| ClientError::InvalidPublicKey(format!("Invalid wallet public key: {}", wallet_pubkey)))?;
        
        let mint = Pubkey::from_str(mint_pubkey)
            .map_err(|_| ClientError::InvalidPublicKey(format!("Invalid mint address: {}", mint_pubkey)))?;
        
        // Create a oneshot channel for the response
        let (response_tx, response_rx) = oneshot::channel();
        
        // Send the command to the transaction processor
        self.tx_sender
            .send(TransactionCommand::GetTokenBalance {
                wallet,
                mint,
                response_tx,
            })
            .await
            .map_err(|e| ClientError::ChannelError(format!("Failed to send transaction command: {}", e)))?;
        
        // Wait for the response
        let balance = response_rx
            .await
            .map_err(|e| ClientError::ChannelError(format!("Failed to receive transaction result: {}", e)))??;
        
        Ok(balance)
    }
    
    // Shutdown the transaction processor
    pub async fn shutdown(&self) -> Result<(), ClientError> {
        info!("Shutting down TokenGatingClient");
        
        // Send the shutdown command
        self.tx_sender
            .send(TransactionCommand::Shutdown)
            .await
            .map_err(|e| ClientError::ChannelError(format!("Failed to send shutdown command: {}", e)))?;
        
        Ok(())
    }
} 