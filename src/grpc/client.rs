use crate::error::AppError;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tracing::info;
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::geyser::SubscribeRequest;
use yellowstone_grpc_proto::prelude::CommitmentLevel;

/// Manages the gRPC connection to Solana RPC via Yellowstone (Triton One's Dragons Mouth).
/// 
/// This client handles connection establishment to Triton One's gRPC streaming service,
/// which provides real-time updates on account activity via Yellowstone gRPC protocol.
pub struct RpcClient {
    grpc_endpoint: String,
    auth_token: String,
    account: Pubkey,
}

impl RpcClient {
    /// Create a new RPC client for the given gRPC endpoint and account.
    /// 
    /// The gRPC endpoint should be in the format: https://host:port
    /// Authentication is provided via the x-token header.
    pub fn new(grpc_endpoint: String, auth_token: String, account: &str) -> Result<Self, AppError> {
        info!(
            grpc_endpoint = %grpc_endpoint,
            account = %account,
            "Creating Yellowstone gRPC client"
        );

        let account = Pubkey::from_str(account)
            .map_err(|e| AppError::Config(format!("Invalid account pubkey: {}", e)))?;

        Ok(Self {
            grpc_endpoint,
            auth_token,
            account,
        })
    }

    /// Connect to the gRPC endpoint and return a configured Yellowstone client.
    /// 
    /// This creates a persistent gRPC connection to monitor all transactions
    /// involving the target account using Triton One's streaming service.
    pub async fn connect(&self) -> Result<GeyserGrpcClient<impl tonic::service::Interceptor>, AppError> {
        info!(
            grpc_endpoint = %self.grpc_endpoint,
            "Connecting to Yellowstone gRPC endpoint"
        );

        // Connect with x-token authentication
        let client = GeyserGrpcClient::build_from_shared(self.grpc_endpoint.clone())
            .map_err(|e| AppError::GrpcConnection(format!("Invalid gRPC endpoint: {}", e)))?
            .x_token(Some(self.auth_token.clone()))
            .map_err(|e| AppError::Config(format!("Invalid auth token: {}", e)))?
            .connect()
            .await
            .map_err(|e| AppError::GrpcConnection(format!("Failed to connect to gRPC endpoint: {}", e)))?;

        info!("Successfully connected to Yellowstone gRPC endpoint");

        Ok(client)
    }

    /// Get the target account pubkey.
    pub fn account(&self) -> &Pubkey {
        &self.account
    }

    /// Create a subscription request for monitoring the target account's transactions.
    /// 
    /// This builds a SubscribeRequest configured to receive updates for all transactions
    /// that mention the target account, excluding vote transactions.
    pub fn create_subscription_request(&self) -> SubscribeRequest {
        use std::collections::HashMap;
        use yellowstone_grpc_proto::geyser::{
            SubscribeRequestFilterAccounts, SubscribeRequestFilterSlots,
            SubscribeRequestFilterTransactions,
        };

        let mut accounts = HashMap::new();
        accounts.insert(
            "target_account".to_string(),
            SubscribeRequestFilterAccounts {
                account: vec![self.account.to_string()],
                owner: vec![],
                filters: vec![],
            },
        );

        let mut transactions = HashMap::new();
        transactions.insert(
            "target_transactions".to_string(),
            SubscribeRequestFilterTransactions {
                vote: Some(false), // Exclude vote transactions
                failed: Some(false), // Exclude failed transactions
                signature: None,
                account_include: vec![self.account.to_string()],
                account_exclude: vec![],
                account_required: vec![],
            },
        );

        let mut slots = HashMap::new();
        slots.insert(
            "slots".to_string(),
            SubscribeRequestFilterSlots {
                filter_by_commitment: Some(true),
            },
        );

        SubscribeRequest {
            accounts,
            slots,
            transactions,
            transactions_status: HashMap::new(),
            blocks: HashMap::new(),
            blocks_meta: HashMap::new(),
            entry: HashMap::new(),
            commitment: Some(CommitmentLevel::Confirmed as i32),
            accounts_data_slice: vec![],
            ping: None,
        }
    }
}
