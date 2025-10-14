use crate::error::AppError;
use crate::solana::models::{BalanceChange, ParsedTransaction};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::Client;
use tracing::{debug, warn};

/// Repository for persisting transaction data to PostgreSQL.
/// 
/// This struct encapsulates all database operations related to transactions
/// and balance changes, providing a clean abstraction over the underlying
/// SQL queries.
#[derive(Clone)]
pub struct TransactionRepository {
    client: Arc<Mutex<Client>>,
}

impl TransactionRepository {
    /// Create a new repository instance with the given client.
    pub fn new(client: Client) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
        }
    }

    /// Insert a transaction into the database.
    /// 
    /// This performs an INSERT operation on the transactions table. If a transaction
    /// with the same signature already exists, it will be skipped (ON CONFLICT DO NOTHING).
    /// This ensures idempotency in case we receive duplicate transaction events.
    /// 
    /// Returns the database ID of the inserted transaction, or None if it was a duplicate.
    pub async fn insert_transaction(
        &self,
        tx: &ParsedTransaction,
    ) -> Result<Option<i64>, AppError> {
        let client = self.client.lock().await;

        let result = client
            .query_opt(
                r#"
                INSERT INTO transactions (
                    signature,
                    slot,
                    block_time,
                    fee,
                    fee_payer,
                    success,
                    compute_units_consumed
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (signature) DO NOTHING
                RETURNING id
                "#,
                &[
                    &tx.signature,
                    &(tx.slot as i64),
                    &tx.block_time,
                    &(tx.fee as i64),
                    &tx.fee_payer,
                    &tx.success,
                    &tx.compute_units_consumed.map(|u| u as i64),
                ],
            )
            .await
            .map_err(|e| AppError::Database(format!("Failed to insert transaction: {}", e)))?;

        match result {
            Some(row) => {
                let id: i64 = row.get(0);
                debug!(
                    signature = %tx.signature,
                    transaction_id = id,
                    "Inserted transaction into database"
                );
                Ok(Some(id))
            }
            None => {
                debug!(
                    signature = %tx.signature,
                    "Duplicate transaction skipped"
                );
                Ok(None)
            }
        }
    }

    /// Insert balance changes associated with a transaction.
    /// 
    /// This inserts all balance changes for a given transaction ID. Balance changes
    /// track how account balances changed as a result of the transaction execution.
    pub async fn insert_balance_changes(
        &self,
        transaction_id: i64,
        changes: &[BalanceChange],
    ) -> Result<(), AppError> {
        if changes.is_empty() {
            return Ok(());
        }

        let client = self.client.lock().await;

        for change in changes {
            let result = client
                .execute(
                    r#"
                    INSERT INTO account_balance_changes (
                        transaction_id,
                        account_address,
                        mint_address,
                        pre_balance,
                        post_balance,
                        balance_delta
                    )
                    VALUES ($1, $2, $3, $4, $5, $6)
                    "#,
                    &[
                        &transaction_id,
                        &change.account_address,
                        &change.mint_address,
                        &change.pre_balance,
                        &change.post_balance,
                        &change.delta(),
                    ],
                )
                .await;

            if let Err(e) = result {
                warn!(
                    transaction_id = transaction_id,
                    error = %e,
                    "Failed to insert balance change, continuing with others"
                );
            }
        }

        debug!(
            transaction_id = transaction_id,
            balance_changes_count = changes.len(),
            "Inserted balance changes"
        );

        Ok(())
    }

    /// Insert a complete parsed transaction with all its balance changes.
    /// 
    /// This is a convenience method that combines transaction insertion with
    /// balance change insertion in a single operation. It ensures data consistency
    /// by using the returned transaction ID to link balance changes.
    pub async fn insert_complete_transaction(
        &self,
        tx: &ParsedTransaction,
    ) -> Result<(), AppError> {
        if let Some(transaction_id) = self.insert_transaction(tx).await? {
            self.insert_balance_changes(transaction_id, &tx.balance_changes)
                .await?;
        }

        Ok(())
    }
}
