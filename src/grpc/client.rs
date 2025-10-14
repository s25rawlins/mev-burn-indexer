use crate::error::AppError;
use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tracing::info;

/// Manages the WebSocket connection to Solana RPC for account monitoring.
/// 
/// This client handles connection establishment and provides access to
/// subscribe to account activity via Solana's PubSub API.
pub struct RpcClient {
    ws_url: String,
    account: Pubkey,
}

impl RpcClient {
    /// Create a new RPC client for the given WebSocket URL and account.
    /// 
    /// The WebSocket URL should be in the format: ws://host:port or wss://host:port
    pub fn new(ws_url: String, account: &str) -> Result<Self, AppError> {
        info!(ws_url = %ws_url, account = %account, "Creating RPC client");

        let account = Pubkey::from_str(account)
            .map_err(|e| AppError::Config(format!("Invalid account pubkey: {}", e)))?;

        Ok(Self { ws_url, account })
    }

    /// Connect to the WebSocket endpoint and subscribe to account logs.
    /// 
    /// This creates a persistent WebSocket connection to monitor all transactions
    /// involving the target account.
    pub async fn connect(&self) -> Result<PubsubClient, AppError> {
        info!(ws_url = %self.ws_url, "Connecting to Solana RPC WebSocket");

        let client = PubsubClient::new(&self.ws_url)
            .await
            .map_err(|e| AppError::GrpcConnection(format!("Failed to connect to WebSocket: {}", e)))?;

        info!("Successfully connected to Solana RPC WebSocket");

        Ok(client)
    }

    /// Get the target account pubkey.
    pub fn account(&self) -> &Pubkey {
        &self.account
    }
}
