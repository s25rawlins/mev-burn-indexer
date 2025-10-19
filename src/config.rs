use crate::error::AppError;
use std::env;

/// Application configuration loaded from environment variables.
/// 
/// All configuration values are validated during construction to fail fast
/// if the environment is misconfigured.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub grpc_endpoint: String,
    pub grpc_token: String,
    pub rpc_http_url: String,
    pub target_account: String,
    pub database_url: String,
    pub log_level: String,
    pub metrics_port: u16,
    pub include_failed_transactions: bool,
}

impl AppConfig {
    /// Load configuration from environment variables.
    /// 
    /// Required environment variables:
    /// - GRPC_ENDPOINT: The gRPC endpoint URL (WebSocket)
    /// - GRPC_TOKEN: Authentication token for RPC services
    /// - TARGET_ACCOUNT: Solana account address to monitor
    /// - DATABASE_URL: PostgreSQL connection string
    /// 
    /// Optional environment variables:
    /// - RPC_HTTP_URL: HTTP RPC endpoint (defaults to public Solana mainnet)
    /// - LOG_LEVEL: Logging level (default: "info")
    /// - METRICS_PORT: Port for Prometheus metrics server (default: 9090)
    /// - INCLUDE_FAILED_TRANSACTIONS: Whether to include failed transactions (default: "true")
    pub fn from_env() -> Result<Self, AppError> {
        let grpc_endpoint = env::var("GRPC_ENDPOINT")
            .map_err(|_| AppError::Config("GRPC_ENDPOINT not set".to_string()))?;

        let grpc_token = env::var("GRPC_TOKEN")
            .map_err(|_| AppError::Config("GRPC_TOKEN not set".to_string()))?;

        let target_account = env::var("TARGET_ACCOUNT")
            .map_err(|_| AppError::Config("TARGET_ACCOUNT not set".to_string()))?;

        let database_url = env::var("DATABASE_URL")
            .map_err(|_| AppError::Config("DATABASE_URL not set".to_string()))?;

        // HTTP RPC endpoint with fallback to public Solana mainnet
        let rpc_http_url = env::var("RPC_HTTP_URL")
            .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());

        let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

        // Parse metrics port with validation
        let metrics_port = env::var("METRICS_PORT")
            .ok()
            .and_then(|port_str| port_str.parse::<u16>().ok())
            .unwrap_or(9090);

        // Parse include_failed_transactions flag
        // Default to true to capture comprehensive data about bot operations
        let include_failed_transactions = env::var("INCLUDE_FAILED_TRANSACTIONS")
            .ok()
            .and_then(|val| val.parse::<bool>().ok())
            .unwrap_or(true);

        // Validate target account is a valid base58 string
        Self::validate_base58_address(&target_account)?;

        // Validate gRPC endpoint URL has correct protocol scheme
        Self::validate_grpc_url(&grpc_endpoint)?;

        Ok(Self {
            grpc_endpoint,
            grpc_token,
            rpc_http_url,
            target_account,
            database_url,
            log_level,
            metrics_port,
            include_failed_transactions,
        })
    }

    /// Validate that a string is a valid base58-encoded Solana address.
    fn validate_base58_address(address: &str) -> Result<(), AppError> {
        bs58::decode(address)
            .into_vec()
            .map_err(|e| AppError::Config(format!("Invalid base58 address: {}", e)))?;
        Ok(())
    }

    /// Validate that the endpoint URL uses the HTTPS protocol for gRPC.
    /// 
    /// gRPC connections require http:// or https:// protocol schemes.
    /// This validation catches configuration errors early rather than
    /// failing during connection attempts.
    fn validate_grpc_url(url: &str) -> Result<(), AppError> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(AppError::Config(
                format!(
                    "GRPC_ENDPOINT must be an HTTP/HTTPS URL (http:// or https://), got: {}",
                    url
                )
            ));
        }
        Ok(())
    }
}
