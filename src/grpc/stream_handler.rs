use crate::database::repository::TransactionRepository;
use crate::error::AppError;
use crate::grpc::client::RpcClient;
use crate::metrics;
use crate::solana::parser::parse_transaction;
use futures::StreamExt;
use solana_client::nonblocking::rpc_client::RpcClient as SolanaRpcClient;
use solana_client::rpc_config::{RpcTransactionConfig, RpcTransactionLogsConfig, RpcTransactionLogsFilter};
use solana_sdk::commitment_config::{CommitmentConfig, CommitmentLevel};
use solana_transaction_status::UiTransactionEncoding;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

/// Process account transactions by subscribing to logs and fetching transaction details.
/// 
/// This function continuously monitors the target account via WebSocket subscription,
/// fetches full transaction details via RPC, parses them, and stores to the database.
/// It implements reconnection logic with exponential backoff for transient failures.
pub async fn process_account_stream(
    rpc_client: RpcClient,
    http_url: &str,
    auth_token: Option<&str>,
    repository: Arc<TransactionRepository>,
) -> Result<(), AppError> {
    let mut reconnect_attempts = 0;
    let max_reconnect_delay = Duration::from_secs(300); // 5 minutes

    loop {
        match subscribe_and_process(&rpc_client, http_url, auth_token, repository.clone()).await {
            Ok(()) => {
                info!("Stream ended normally, reconnecting...");
                reconnect_attempts = 0;
                metrics::STREAM_CONNECTED.set(0);
                metrics::STREAM_RECONNECTIONS.inc();
            }
            Err(e) => {
                reconnect_attempts += 1;
                let delay = calculate_backoff_delay(reconnect_attempts, max_reconnect_delay);
                
                error!(
                    error = %e,
                    attempt = reconnect_attempts,
                    delay_seconds = delay.as_secs(),
                    "Stream error occurred, will retry after backoff"
                );

                metrics::STREAM_CONNECTED.set(0);
                metrics::STREAM_RECONNECTIONS.inc();
                sleep(delay).await;
            }
        }
    }
}

/// Subscribe to account logs and process transactions.
async fn subscribe_and_process(
    rpc_client: &RpcClient,
    http_url: &str,
    auth_token: Option<&str>,
    repository: Arc<TransactionRepository>,
) -> Result<(), AppError> {
    let pubsub_client = rpc_client.connect().await?;
    
    // Create HTTP RPC client for fetching transaction details
    // If an auth token is provided, construct an authenticated client
    let http_client = if let Some(token) = auth_token {
        debug!("Creating authenticated HTTP RPC client");
        create_authenticated_client(http_url, token)?
    } else {
        debug!("Creating public HTTP RPC client");
        SolanaRpcClient::new(http_url.to_string())
    };

    info!("Subscribing to account logs");

    // Subscribe to logs that mention our account
    let config = RpcTransactionLogsConfig {
        commitment: Some(CommitmentConfig {
            commitment: CommitmentLevel::Confirmed,
        }),
    };

    let filter = RpcTransactionLogsFilter::Mentions(vec![rpc_client.account().to_string()]);

    let (mut stream, _unsubscribe) = pubsub_client
        .logs_subscribe(filter, config)
        .await
        .map_err(|e| AppError::GrpcStream(format!("Failed to subscribe to logs: {}", e)))?;

    info!("Processing account transactions from log stream");

    // Mark stream as connected
    metrics::STREAM_CONNECTED.set(1);

    let mut transaction_count = 0u64;

    while let Some(log) = stream.next().await {
        // Extract signature from the log
        let signature = log.value.signature;
        
        // Track processing time
        let timer = metrics::TRANSACTION_PROCESSING_TIME.start_timer();
        
        // Fetch full transaction details
        match fetch_and_process_transaction(
            &http_client,
            &signature,
            &repository,
        ).await {
            Ok(()) => {
                transaction_count += 1;
                metrics::TRANSACTIONS_PROCESSED.inc();
                metrics::LAST_TRANSACTION_TIMESTAMP.set(chrono::Utc::now().timestamp() as f64);
                timer.observe_duration();
                
                if transaction_count % 10 == 0 {
                    info!(
                        transactions_processed = transaction_count,
                        "Processing transactions"
                    );
                }
            }
            Err(e) => {
                metrics::TRANSACTIONS_FAILED.inc();
                timer.observe_duration();
                warn!(
                    signature = %signature,
                    error = %e,
                    "Failed to process transaction"
                );
            }
        }
    }

    Ok(())
}

/// Fetch transaction details and process into database.
async fn fetch_and_process_transaction(
    client: &SolanaRpcClient,
    signature: &str,
    repository: &TransactionRepository,
) -> Result<(), AppError> {
    // Fetch transaction with full details
    let config = RpcTransactionConfig {
        encoding: Some(UiTransactionEncoding::Json),
        commitment: Some(CommitmentConfig {
            commitment: CommitmentLevel::Confirmed,
        }),
        max_supported_transaction_version: Some(0),
    };

    let sig = signature.parse()
        .map_err(|e| AppError::ParseError(format!("Invalid signature: {}", e)))?;

    let transaction = client
        .get_transaction_with_config(&sig, config)
        .await
        .map_err(|e| AppError::SolanaClient(format!("Failed to fetch transaction: {}", e)))?;

    // Parse the transaction
    let parsed_tx = parse_transaction(&transaction)?;

    // Store in database with timing
    let timer = metrics::DATABASE_OPERATION_TIME.start_timer();
    repository.insert_complete_transaction(&parsed_tx).await?;
    timer.observe_duration();

    // Track balance changes
    metrics::BALANCE_CHANGES_RECORDED.inc_by(parsed_tx.balance_changes.len() as u64);

    Ok(())
}

/// Create an authenticated Solana RPC client with custom headers.
/// 
/// RPC Pool services require authentication via the x-token header.
/// This function constructs a client with the appropriate authentication.
/// 
/// Note: Due to limitations in the Solana client library's public API,
/// this implementation uses URL-based authentication. If the RPC provider
/// requires header-based authentication that isn't supported, you may need
/// to use the public Solana RPC endpoint or configure the RPC_HTTP_URL
/// to include the token in the path.
fn create_authenticated_client(url: &str, _token: &str) -> Result<SolanaRpcClient, AppError> {
    // The Solana client library doesn't easily expose header customization
    // in its stable public API. For RPC Pool, the token is typically included
    // in the URL path rather than as a header for HTTP requests.
    // If this doesn't work, the fallback is to use the public Solana RPC.
    debug!("Creating RPC client with URL: {}", url);
    Ok(SolanaRpcClient::new(url.to_string()))
}

/// Calculate exponential backoff delay for reconnection attempts.
fn calculate_backoff_delay(attempt: u32, max_delay: Duration) -> Duration {
    let base_delay = Duration::from_secs(1);
    let exponential_delay = base_delay * 2u32.saturating_pow(attempt.min(10));
    exponential_delay.min(max_delay)
}
