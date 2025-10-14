use crate::database::repository::TransactionRepository;
use crate::error::AppError;
use crate::grpc::client::RpcClient;
use crate::metrics;
use crate::solana::parser::parse_transaction;
use futures::{SinkExt, StreamExt};
use solana_client::nonblocking::rpc_client::RpcClient as SolanaRpcClient;
use solana_sdk::commitment_config::{CommitmentConfig, CommitmentLevel};
use solana_transaction_status::UiTransactionEncoding;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;

/// Process account transactions by subscribing to Yellowstone gRPC stream.
/// 
/// This function continuously monitors the target account via gRPC subscription,
/// fetches full transaction details via RPC, parses them, and stores to the database.
/// It implements reconnection logic with exponential backoff for transient failures.
pub async fn process_account_stream(
    rpc_client: RpcClient,
    http_url: &str,
    repository: Arc<TransactionRepository>,
) -> Result<(), AppError> {
    let mut reconnect_attempts = 0;
    let max_reconnect_delay = Duration::from_secs(300); // 5 minutes

    loop {
        match subscribe_and_process(&rpc_client, http_url, repository.clone()).await {
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

/// Subscribe to gRPC stream and process transaction updates.
async fn subscribe_and_process(
    rpc_client: &RpcClient,
    http_url: &str,
    repository: Arc<TransactionRepository>,
) -> Result<(), AppError> {
    // Connect to Yellowstone gRPC
    let mut geyser_client = rpc_client.connect().await?;
    
    // Create HTTP RPC client for fetching full transaction details
    debug!("Creating HTTP RPC client for transaction fetching");
    let http_client = SolanaRpcClient::new(http_url.to_string());

    info!("Subscribing to Yellowstone gRPC stream");

    // Create subscription request
    let request = rpc_client.create_subscription_request();

    // Subscribe to the stream
    let (mut subscribe_tx, mut stream) = geyser_client
        .subscribe()
        .await
        .map_err(|e| AppError::GrpcStream(format!("Failed to create subscription: {}", e)))?;

    // Send the subscription request
    subscribe_tx
        .send(request)
        .await
        .map_err(|e| AppError::GrpcStream(format!("Failed to send subscription request: {}", e)))?;

    info!("Processing transaction updates from gRPC stream");

    // Mark stream as connected
    metrics::STREAM_CONNECTED.set(1);

    let mut transaction_count = 0u64;
    let mut last_ping = tokio::time::Instant::now();
    let ping_interval = Duration::from_secs(30);

    while let Some(message) = stream.next().await {
        // Handle potential stream errors
        let update = message
            .map_err(|e| AppError::GrpcStream(format!("Stream error: {}", e)))?;

        // Send periodic pings to keep the connection alive
        if last_ping.elapsed() >= ping_interval {
            send_ping(&mut subscribe_tx).await?;
            last_ping = tokio::time::Instant::now();
        }

        // Process the update based on its type
        match update.update_oneof {
            Some(UpdateOneof::Transaction(transaction_update)) => {
                // Extract transaction signature
                let signature = if let Some(tx) = &transaction_update.transaction {
                    if !tx.signature.is_empty() {
                        bs58::encode(&tx.signature).into_string()
                    } else {
                        warn!("Transaction update missing signature");
                        continue;
                    }
                } else {
                    warn!("Transaction update missing transaction data");
                    continue;
                };

                // Track processing time
                let timer = metrics::TRANSACTION_PROCESSING_TIME.start_timer();

                // Fetch and process full transaction details
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
            Some(UpdateOneof::Slot(slot_update)) => {
                debug!(
                    slot = slot_update.slot,
                    status = ?slot_update.status,
                    "Received slot update"
                );
            }
            Some(UpdateOneof::Pong(_)) => {
                debug!("Received pong response");
            }
            _ => {
                // Ignore other update types (account, block, etc.)
            }
        }
    }

    Ok(())
}

/// Send a ping message to keep the stream alive.
async fn send_ping<S>(subscribe_tx: &mut S) -> Result<(), AppError>
where
    S: SinkExt<yellowstone_grpc_proto::geyser::SubscribeRequest> + Unpin,
    S::Error: std::fmt::Display,
{
    use yellowstone_grpc_proto::geyser::{SubscribeRequest, SubscribeRequestPing};
    
    let ping_request = SubscribeRequest {
        ping: Some(SubscribeRequestPing { id: 1 }),
        ..Default::default()
    };

    subscribe_tx
        .send(ping_request)
        .await
        .map_err(|e| AppError::GrpcStream(format!("Failed to send ping: {}", e)))?;

    Ok(())
}

/// Fetch transaction details and process into database.
async fn fetch_and_process_transaction(
    client: &SolanaRpcClient,
    signature: &str,
    repository: &TransactionRepository,
) -> Result<(), AppError> {
    use solana_client::rpc_config::RpcTransactionConfig;

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

/// Calculate exponential backoff delay for reconnection attempts.
fn calculate_backoff_delay(attempt: u32, max_delay: Duration) -> Duration {
    let base_delay = Duration::from_secs(1);
    let exponential_delay = base_delay * 2u32.saturating_pow(attempt.min(10));
    exponential_delay.min(max_delay)
}
