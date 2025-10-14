use crate::error::AppError;
use crate::solana::models::{BalanceChange, ParsedTransaction};
use chrono::{DateTime, Utc};
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;
use tracing::{debug, warn};

/// Parse a Solana transaction from the RPC response into our domain model.
/// 
/// This function extracts all relevant fields including fee, signature, block time,
/// success status, and balance changes from the transaction returned by the RPC client.
pub fn parse_transaction(
    encoded_tx: &EncodedConfirmedTransactionWithStatusMeta,
) -> Result<ParsedTransaction, AppError> {
    let slot = encoded_tx.slot;
    
    // Extract block time and convert to DateTime
    let block_time = encoded_tx.block_time.map(|timestamp| {
        DateTime::from_timestamp(timestamp, 0)
            .unwrap_or_else(|| DateTime::<Utc>::MIN_UTC)
    });

    // Extract transaction metadata
    let meta = encoded_tx
        .transaction
        .meta
        .as_ref()
        .ok_or_else(|| AppError::ParseError("Transaction missing metadata".to_string()))?;

    let fee = meta.fee;

    // Determine if transaction succeeded (err field should be None)
    let success = meta.err.is_none();

    // Extract compute units consumed if available
    let compute_units_consumed: Option<u64> = meta.compute_units_consumed.clone().into();

    // Extract the transaction signature
    let transaction = &encoded_tx.transaction.transaction;
    let signature = match transaction {
        solana_transaction_status::EncodedTransaction::Json(ui_tx) => {
            ui_tx.signatures.first()
                .ok_or_else(|| AppError::ParseError("Transaction has no signature".to_string()))?
                .clone()
        }
        _ => {
            return Err(AppError::ParseError("Unsupported transaction encoding".to_string()));
        }
    };

    // Extract fee payer from the transaction
    let fee_payer = match transaction {
        solana_transaction_status::EncodedTransaction::Json(ui_tx) => {
            // Get the first account key which is the fee payer
            match &ui_tx.message {
                solana_transaction_status::UiMessage::Parsed(parsed) => {
                    parsed.account_keys.first()
                        .map(|key| key.pubkey.clone())
                        .ok_or_else(|| AppError::ParseError("No account keys in transaction".to_string()))?
                }
                solana_transaction_status::UiMessage::Raw(raw) => {
                    raw.account_keys.first()
                        .cloned()
                        .ok_or_else(|| AppError::ParseError("No account keys in transaction".to_string()))?
                }
            }
        }
        _ => {
            return Err(AppError::ParseError("Cannot determine fee payer".to_string()));
        }
    };

    // Extract balance changes
    let balance_changes = extract_balance_changes(transaction, meta)?;

    debug!(
        signature = %signature,
        slot = slot,
        fee = fee,
        success = success,
        "Parsed transaction"
    );

    Ok(ParsedTransaction {
        signature,
        slot,
        block_time,
        fee,
        fee_payer,
        success,
        compute_units_consumed,
        balance_changes,
    })
}

/// Extract balance changes from transaction metadata.
/// 
/// This compares pre_balances and post_balances arrays to calculate the net change
/// for each account involved in the transaction. SPL token balance changes are
/// also extracted from pre_token_balances and post_token_balances if available.
fn extract_balance_changes(
    transaction: &solana_transaction_status::EncodedTransaction,
    meta: &solana_transaction_status::UiTransactionStatusMeta,
) -> Result<Vec<BalanceChange>, AppError> {
    let mut balance_changes = Vec::new();

    // Extract account keys based on message type
    let account_keys = match transaction {
        solana_transaction_status::EncodedTransaction::Json(ui_tx) => {
            match &ui_tx.message {
                solana_transaction_status::UiMessage::Parsed(parsed) => {
                    parsed.account_keys.iter().map(|k| k.pubkey.clone()).collect()
                }
                solana_transaction_status::UiMessage::Raw(raw) => {
                    raw.account_keys.clone()
                }
            }
        }
        _ => {
            warn!("Cannot extract balance changes from non-JSON transaction format");
            return Ok(balance_changes);
        }
    };

    // Process native SOL balance changes
    for (index, (pre_balance, post_balance)) in meta
        .pre_balances
        .iter()
        .zip(meta.post_balances.iter())
        .enumerate()
    {
        // Only record if there was a change
        if pre_balance != post_balance {
            let account_address = account_keys
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("unknown_{}", index));

            balance_changes.push(BalanceChange {
                account_address,
                mint_address: None, // None indicates native SOL
                pre_balance: *pre_balance as i64,
                post_balance: *post_balance as i64,
            });
        }
    }

    // Process SPL token balance changes if available
    // Note: In Solana 1.18, these fields use OptionSerializer which implements Into<Option>
    use solana_transaction_status::UiTransactionTokenBalance;
    let pre_token_opt: Option<Vec<UiTransactionTokenBalance>> = meta.pre_token_balances.clone().into();
    let post_token_opt: Option<Vec<UiTransactionTokenBalance>> = meta.post_token_balances.clone().into();
    
    if let (Some(ref pre_token_balances), Some(ref post_token_balances)) = (
        pre_token_opt,
        post_token_opt,
    ) {
        for pre_token in pre_token_balances {
            // Find matching post token balance by account index
            if let Some(post_token) = post_token_balances
                .iter()
                .find(|pt| pt.account_index == pre_token.account_index)
            {
                let account_address = account_keys
                    .get(pre_token.account_index as usize)
                    .cloned()
                    .unwrap_or_else(|| format!("unknown_{}", pre_token.account_index));

                let pre_amount = pre_token
                    .ui_token_amount
                    .amount
                    .parse::<i64>()
                    .unwrap_or(0);
                let post_amount = post_token
                    .ui_token_amount
                    .amount
                    .parse::<i64>()
                    .unwrap_or(0);

                // Only record if there was a change
                if pre_amount != post_amount {
                    balance_changes.push(BalanceChange {
                        account_address,
                        mint_address: Some(pre_token.mint.clone()),
                        pre_balance: pre_amount,
                        post_balance: post_amount,
                    });
                }
            }
        }
    }

    Ok(balance_changes)
}
