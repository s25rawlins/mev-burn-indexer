use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents a parsed Solana transaction with all relevant metadata.
/// 
/// This struct contains the essential information extracted from a raw
/// Solana transaction that we need to persist to the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTransaction {
    /// Base58-encoded transaction signature (unique identifier)
    pub signature: String,
    
    /// Slot number in which this transaction was processed
    pub slot: u64,
    
    /// Unix timestamp of the block (may be None for unconfirmed transactions)
    pub block_time: Option<DateTime<Utc>>,
    
    /// Transaction fee paid in lamports (1 SOL = 1,000,000,000 lamports)
    pub fee: u64,
    
    /// Base58-encoded public key of the account that paid the fee
    pub fee_payer: String,
    
    /// Whether the transaction executed successfully
    pub success: bool,
    
    /// Compute units consumed by this transaction (may be None if not available)
    pub compute_units_consumed: Option<u64>,
    
    /// Account balance changes that occurred during this transaction
    pub balance_changes: Vec<BalanceChange>,
}

/// Represents a change in an account's balance during a transaction.
/// 
/// This captures the pre and post-transaction balance for an account,
/// allowing us to track token movements and calculate PnL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceChange {
    /// Base58-encoded address of the account whose balance changed
    pub account_address: String,
    
    /// Base58-encoded mint address for SPL tokens (None for native SOL)
    pub mint_address: Option<String>,
    
    /// Balance before the transaction (in smallest unit: lamports for SOL, token units for SPL)
    pub pre_balance: i64,
    
    /// Balance after the transaction
    pub post_balance: i64,
}

impl BalanceChange {
    /// Calculate the net change in balance (post - pre).
    /// 
    /// Positive values indicate an increase, negative values indicate a decrease.
    pub fn delta(&self) -> i64 {
        self.post_balance - self.pre_balance
    }
}
