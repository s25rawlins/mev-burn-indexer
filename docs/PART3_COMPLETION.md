# Part 3, Expand: Implementation documentation

## Overview

Part 3 of the MEV Burn Indexer extends data ingestion and visualization beyond basic transaction fees. You'll now have comprehensive profit and loss analysis with detailed balance change tracking.

## Additional data points ingested

### 1. Account balance changes

The `account_balance_changes` table captures detailed balance information for all accounts involved in each transaction.

**Schema:**
```sql
CREATE TABLE account_balance_changes (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BIGINT NOT NULL REFERENCES transactions(id),
    account_address VARCHAR(44) NOT NULL,
    mint_address VARCHAR(44),  -- NULL for SOL, populated for SPL tokens
    pre_balance BIGINT NOT NULL,
    post_balance BIGINT NOT NULL,
    balance_delta BIGINT NOT NULL  -- Precomputed: post_balance - pre_balance
);
```

**Key features:**
- Tracks both SOL (native) and SPL token balance changes
- Includes pre and post transaction balances for verification
- Precomputed balance_delta field optimizes aggregation queries
- Foreign key relationship ensures referential integrity

### 2. Transaction success status

The `transactions` table includes a `success` field derived from Solana transaction metadata:

```sql
success BOOLEAN NOT NULL  -- true if transaction succeeded, false if failed
```

This field allows filtering profitable vs unprofitable transactions and analyzing failure patterns.

### 3. Compute units consumed

The `transactions` table tracks computational resources:

```sql
compute_units_consumed BIGINT  -- Computational resources used by the transaction
```

You can use this data point to analyze:
- Transaction complexity
- Cost efficiency (fees vs compute units)
- Bot optimization opportunities

## Visualizations created

### Dashboard: MEV Burn Analysis, Expanded (Part 3)

Location: `monitoring/grafana/dashboards/mev-burn-expanded.json`

This dashboard provides comprehensive profit and loss analysis with balance change visualization.

#### 1. Summary statistics (top row)

**Total Burn (Lamports)**
Displays the aggregate of all transaction fees paid by the bot. Color coding shows thresholds: green for values under 1M, yellow between 1M and 10M, red above 10M.

**Net SOL Change**
Shows total SOL balance change across all successful transactions. You can see at a glance whether the bot is gaining or losing SOL overall. Red indicates losses, yellow shows breaking even, green means profitable.

**Estimated PnL**
Calculates net profit after accounting for transaction costs using the formula: `SUM(balance_delta) - SUM(fees)`. This is your key metric for bot profitability assessment.

#### 2. SOL balance changes over time

**Chart type:** Time series (line chart)
**Time range:** Last 7 days, hourly aggregation
**Series:**
- SOL Gains (green): Positive balance changes
- SOL Losses (red): Negative balance changes

**SQL query:**
```sql
-- Gains
SELECT 
  DATE_TRUNC('hour', t.block_time) as time,
  SUM(CASE WHEN bc.balance_delta > 0 THEN bc.balance_delta ELSE 0 END) as "SOL Gains"
FROM account_balance_changes bc
JOIN transactions t ON bc.transaction_id = t.id
WHERE bc.mint_address IS NULL
  AND t.success = true
  AND t.block_time >= NOW() - INTERVAL '7 days'
GROUP BY time
ORDER BY time;

-- Losses query is similar with balance_delta < 0
```

**Use cases:**
- Identify trading patterns and peak activity periods
- Detect unusual losses or gains
- Correlate balance changes with market conditions

#### 3. Cumulative PnL vs fees

**Chart type:** Time series (line chart)
**Time range:** Last 7 days, hourly aggregation
**Series:**
- Cumulative PnL (blue): Running total of profit and loss
- Cumulative Fees (orange): Running total of fees paid

**SQL query:**
```sql
SELECT 
  DATE_TRUNC('hour', t.block_time) as time,
  SUM(SUM(bc.balance_delta - t.fee)) OVER (ORDER BY DATE_TRUNC('hour', t.block_time)) as "Cumulative PnL"
FROM account_balance_changes bc
JOIN transactions t ON bc.transaction_id = t.id
WHERE bc.mint_address IS NULL
  AND t.success = true
  AND t.block_time >= NOW() - INTERVAL '7 days'
GROUP BY time
ORDER BY time;
```

**Analysis insights:**
You can see the bot's profitability trajectory over time, understand the relationship between fees paid and profits earned, identify periods of high cost vs high profit, and assess whether fee optimization is needed.

#### 4. Largest SOL balance changes

**Chart type:** Table
**Rows:** Top 50 transactions by absolute balance change value
**Columns:**
- Transaction signature (clickable link to Solscan)
- Block time
- Account address
- Balance change (color coded background)

**Color coding:**
Red background indicates negative changes (losses), yellow shows small negative changes, green displays positive changes (gains).

**SQL query:**
```sql
SELECT 
  t.signature,
  t.block_time,
  bc.account_address,
  bc.balance_delta
FROM account_balance_changes bc
JOIN transactions t ON bc.transaction_id = t.id
WHERE bc.mint_address IS NULL
  AND t.success = true
ORDER BY ABS(bc.balance_delta) DESC
LIMIT 50;
```

**Use cases:**
You can identify the largest profitable trades, investigate significant losses, analyze account specific patterns, and deep dive into individual transactions via Solscan links.

## Data analysis capabilities

### Profit and loss calculation

The system calculates estimated PnL using this formula:

```
PnL = Total SOL Gained, Total SOL Spent, Total Fees
```

Where:
- **Total SOL Gained:** Sum of positive balance deltas
- **Total SOL Spent:** Sum of negative balance deltas (trading costs, swaps)
- **Total Fees:** Transaction fees paid to the Solana network

**Query example:**
```sql
SELECT 
  DATE(t.block_time) as date,
  SUM(CASE WHEN bc.balance_delta > 0 THEN bc.balance_delta ELSE 0 END) as gains,
  SUM(CASE WHEN bc.balance_delta < 0 THEN ABS(bc.balance_delta) ELSE 0 END) as costs,
  SUM(t.fee) as fees,
  SUM(bc.balance_delta) - SUM(t.fee) as net_pnl
FROM account_balance_changes bc
JOIN transactions t ON bc.transaction_id = t.id
WHERE bc.mint_address IS NULL
  AND t.success = true
GROUP BY DATE(t.block_time)
ORDER BY date DESC;
```

### Balance change analysis

You can track how account balances evolve over time:

```sql
-- Account specific balance evolution
SELECT 
  t.block_time,
  bc.account_address,
  bc.pre_balance,
  bc.post_balance,
  bc.balance_delta
FROM account_balance_changes bc
JOIN transactions t ON bc.transaction_id = t.id
WHERE bc.account_address = 'specific_address_here'
  AND bc.mint_address IS NULL
ORDER BY t.block_time;
```

### Token specific analysis

The schema supports tracking SPL token balance changes:

```sql
-- USDC balance changes (example)
SELECT 
  t.signature,
  t.block_time,
  bc.account_address,
  bc.balance_delta,
  bc.mint_address
FROM account_balance_changes bc
JOIN transactions t ON bc.transaction_id = t.id
WHERE bc.mint_address = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v'  -- USDC mint
ORDER BY t.block_time DESC;
```

## Implementation details

### Parser enhancement

The `src/solana/parser.rs` module extracts balance changes from Solana transaction metadata through these steps:

1. **Pre and post balance arrays:** Transaction metadata includes `pre_balances` and `post_balances` arrays indexed by account position
2. **Balance delta calculation:** For each account, the system computes `post_balance, pre_balance`
3. **Token balance handling:** The parser processes `pre_token_balances` and `post_token_balances` separately for SPL tokens
4. **Mint address extraction:** The system identifies token mint addresses to distinguish between different tokens

### Database operations

Balance changes are inserted atomically with their parent transaction:

```rust
pub async fn insert_complete_transaction(&self, tx: &ParsedTransaction) 
    -> Result<(), AppError> 
{
    // Insert transaction
    if let Some(tx_id) = self.insert_transaction(tx).await? {
        // Only insert balance changes for new transactions (not duplicates)
        self.insert_balance_changes(tx_id, &tx.balance_changes).await?;
    }
    Ok(())
}
```

## Future enhancements

### 1. Instruction level analysis

You could add a table to track program instructions called by each transaction:

```sql
CREATE TABLE transaction_instructions (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BIGINT REFERENCES transactions(id),
    instruction_index INT,
    program_id VARCHAR(44),  -- e.g., Raydium, Orca, Jupiter
    instruction_data BYTEA,
    accounts JSONB
);
```

This would enable:
- Identifying which DEXes the bot uses most frequently
- Analyzing instruction patterns for successful vs failed trades
- Detecting strategy changes over time

### 2. Price data integration

You could integrate USD pricing for tokens to calculate actual PnL in dollar terms:

```sql
CREATE TABLE token_prices (
    id BIGSERIAL PRIMARY KEY,
    mint_address VARCHAR(44),
    timestamp TIMESTAMPTZ,
    price_usd NUMERIC(20, 10),
    source VARCHAR(50)  -- e.g., 'coingecko', 'jupiter'
);
```

### 3. Multi account tracking

The system could be extended to monitor multiple trading bots simultaneously:

- Add `bot_account` field to transactions table
- Filter dashboard by bot account using Grafana variables
- Compare performance across multiple bots

### 4. Alerting

You could implement alerts for:
- Large losses (exceeding threshold)
- Success rate drops below target
- Extended periods without profitable trades
- Unusual transaction patterns

## Accessing the dashboard

Follow these steps to view the expanded dashboard:

1. **Start the stack:**
   ```bash
   docker-compose up -d
   ```

2. **Access Grafana:**
   Open http://localhost:3000 in your browser. Use the default credentials: admin/admin.

3. **Navigate to the dashboard:**
   Click "Dashboards" in the sidebar, then select "MEV Burn Analysis, Expanded (Part 3)".

The dashboard auto refreshes every 30 seconds to show you the latest data.

## Summary

Part 3 extends the MEV Burn Indexer with comprehensive balance change tracking and profit and loss analysis. You can now see actionable insights into bot performance and make data driven optimization decisions for trading strategies.
