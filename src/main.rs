mod config;
mod database;
mod error;
mod grpc;
mod metrics;
mod metrics_server;
mod solana;
mod telemetry;

use crate::config::AppConfig;
use crate::database::{connection, repository::TransactionRepository};
use crate::error::AppError;
use crate::grpc::client::RpcClient;
use crate::grpc::stream_handler::process_account_stream;
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Load environment variables from .env file if present
    dotenvy::dotenv().ok();

    // Load and validate configuration
    let config = AppConfig::from_env()?;

    // Initialize telemetry (structured logging)
    telemetry::init_telemetry(&config.log_level);

    // Initialize metrics
    metrics::init_metrics();

    info!("Starting Solana Bot Transaction Tracker");
    info!(
        target_account = %config.target_account,
        grpc_endpoint = %config.grpc_endpoint,
        "Configuration loaded"
    );

    // Establish database connection
    let db_client = connection::create_client(&config.database_url).await?;

    // Run database migrations
    connection::run_migrations(&db_client).await?;

    // Create repository for database operations
    let repository = Arc::new(TransactionRepository::new(db_client));

    // Create RPC client for WebSocket subscription
    let rpc_client = RpcClient::new(config.grpc_endpoint.clone(), &config.target_account)?;

    info!("All systems initialized, starting stream processing");

    // Start metrics server in background
    let metrics_port = 9090;
    tokio::spawn(async move {
        if let Err(e) = metrics_server::start_metrics_server(metrics_port).await {
            tracing::error!("Metrics server error: {}", e);
        }
    });

    // Start uptime tracking
    let start_time = std::time::Instant::now();
    tokio::spawn(async move {
        loop {
            metrics::APP_UPTIME.set(start_time.elapsed().as_secs_f64());
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });

    // Start processing the account stream (runs indefinitely with auto-reconnection)
    // Use the configured HTTP RPC URL and authentication token
    let auth_token = if config.rpc_http_url.contains("rpcpool.com") {
        Some(config.grpc_token.as_str())
    } else {
        None
    };
    
    process_account_stream(
        rpc_client,
        &config.rpc_http_url,
        auth_token,
        repository
    ).await?;

    Ok(())
}
