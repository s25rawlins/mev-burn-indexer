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

    // Create RPC client for Yellowstone gRPC subscription
    let rpc_client = RpcClient::new(
        config.grpc_endpoint.clone(),
        config.grpc_token.clone(),
        &config.target_account,
        config.include_failed_transactions,
    )?;

    if config.include_failed_transactions {
        info!("Configured to capture both successful and failed transactions for comprehensive analysis");
    } else {
        info!("Configured to capture only successful transactions");
    }

    info!("All systems initialized, starting stream processing");

    // Start metrics server in background
    let metrics_port = config.metrics_port;
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
    process_account_stream(
        rpc_client,
        &config.rpc_http_url,
        repository
    ).await?;

    Ok(())
}
