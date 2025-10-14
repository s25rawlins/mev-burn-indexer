# MEV Burn Indexer

A production-grade Rust application that monitors and records transaction activity for a specific Solana trading bot account. The system streams real-time transaction data via gRPC, extracts key metrics including fees and balance changes, and persists this information to PostgreSQL for historical analysis.

## What This Does

This application tracks the `MEViEnscUm6tsQRoGd9h6nLQaQspKj7DB2M5FwM3Xvz` account on Solana, recording every transaction it makes. For each transaction, you'll get:

- Transaction signature and metadata
- Fees paid (the "burn" cost)
- Success/failure status
- Compute units consumed
- Account balance changes (both SOL and SPL tokens)

All data is stored in PostgreSQL with proper indexing for efficient querying and analysis.

## Prerequisites

Before running this application, you'll need:

- **Rust** 1.70 or later ([installation guide](https://rustup.rs/))
- **PostgreSQL** 13 or later (local instance or cloud provider like NeonDB)
- **Database credentials** with permissions to create tables and indexes
- **gRPC access** to temporal.rpcpool.com (credentials provided in requirements)

## Getting Started

### 1. Clone the Repository

```bash
git clone <repository-url>
cd mev-burn-indexer
```

### 2. Set Up the Database

Create a PostgreSQL database for the application:

```bash
createdb mev_burn_indexer
```

Or use your cloud provider's interface to provision a new database.

### 3. Configure Environment Variables

Copy the example environment file and update it with your credentials:

```bash
cp .env.example .env
```

Edit `.env` to match your setup:

```env
GRPC_ENDPOINT=https://temporal.rpcpool.com
GRPC_TOKEN=your-token-here
TARGET_ACCOUNT=MEViEnscUm6tsQRoGd9h6nLQaQspKj7DB2M5FwM3Xvz
DATABASE_URL=postgresql://username:password@host:port/mev_burn_indexer
LOG_LEVEL=info
```

**Database URL format:**
- Local: `postgresql://username:password@localhost:5432/mev_burn_indexer`
- NeonDB: `postgresql://username:password@ep-xyz.neon.tech/mev_burn_indexer?sslmode=require`

### 4. Build the Application

```bash
cargo build --release
```

The release build includes optimizations and will run faster than the debug build.

### 5. Run the Application

**Option 1: Use the Startup Script (Recommended)**

The included startup script handles all prerequisites, builds, and runs the application with detailed logging:

```bash
./start.sh
```

The script will:
- Validate all prerequisites (Rust toolchain, .env file, required variables)
- Build the application in release mode
- Start the application with detailed console logging
- Display configuration summary before startup

To adjust log level:
```bash
LOG_LEVEL=debug ./start.sh
```

**Option 2: Manual Start**

```bash
cargo run --release
```

On first run, the application will automatically apply database migrations to create the required tables. You should see log output confirming:

1. Configuration loaded
2. Database connection established
3. Migrations completed
4. gRPC connection successful
5. Stream processing started

The application will now continuously monitor the target account and save transactions to your database.

## Architecture Overview

### Components

The application is organized into several modules, each with a specific responsibility:

**Configuration (`src/config.rs`)**
- Loads environment variables
- Validates configuration values
- Provides type-safe access to settings

**Database Layer (`src/database/`)**
- `connection.rs`: Manages the PostgreSQL connection pool
- `repository.rs`: Handles all database operations using the repository pattern

**gRPC Client (`src/grpc/`)**
- `client.rs`: Establishes and maintains the connection to the gRPC endpoint
- `stream_handler.rs`: Processes the account update stream with automatic reconnection

**Solana Parser (`src/solana/`)**
- `models.rs`: Domain models for transactions and balance changes
- `parser.rs`: Converts raw transaction data into structured formats

**Error Handling (`src/error.rs`)**
- Custom error types for different failure modes
- Integration with `thiserror` for ergonomic error handling

**Telemetry (`src/telemetry.rs`)**
- Structured logging using the `tracing` framework
- Configurable log levels for debugging and production

### Data Flow

```
gRPC Stream → Parse Transaction → Extract Metadata → Save to Database
     ↓              ↓                    ↓                  ↓
  Account      Signature          Fees, Balances      PostgreSQL
  Updates      Slot, Time         Success Status      (Indexed)
```

The application maintains a persistent connection to the gRPC stream. When a transaction involving the target account occurs, it's parsed and immediately saved to the database. If the connection drops, exponential backoff retry logic automatically reconnects.

### Database Schema

**transactions**
- Stores core transaction metadata
- Indexed on signature (unique), slot, block_time, and fee_payer
- Tracks fees paid and compute units consumed

**account_balance_changes**
- Records how account balances changed during each transaction
- Links to transactions via foreign key
- Supports both SOL and SPL token balance changes
- Indexed for efficient account and mint lookups

## Usage Examples

### Querying Total Fees Paid

```sql
SELECT 
    DATE_TRUNC('day', ingested_at) as date,
    SUM(fee) / 1000000000.0 as total_sol_burned
FROM transactions
WHERE success = true
GROUP BY DATE_TRUNC('day', ingested_at)
ORDER BY date DESC;
```

### Finding Largest Balance Changes

```sql
SELECT 
    t.signature,
    t.block_time,
    bc.account_address,
    bc.balance_delta / 1000000000.0 as sol_change
FROM account_balance_changes bc
JOIN transactions t ON bc.transaction_id = t.id
WHERE bc.mint_address IS NULL  -- SOL only
ORDER BY ABS(bc.balance_delta) DESC
LIMIT 10;
```

### Transaction Success Rate

```sql
SELECT 
    COUNT(*) FILTER (WHERE success = true) as successful,
    COUNT(*) FILTER (WHERE success = false) as failed,
    ROUND(100.0 * COUNT(*) FILTER (WHERE success = true) / COUNT(*), 2) as success_rate
FROM transactions;
```

## Monitoring and Observability

### Grafana Dashboard

The application includes a complete monitoring stack with Prometheus and Grafana. See [MONITORING.md](MONITORING.md) for the full setup guide.

**Quick Start:**
1. Start your application with `./start.sh`
2. Start monitoring stack: `docker-compose -f docker-compose.monitoring.yml up -d`
3. Open Grafana at http://localhost:3000 (admin/admin)
4. View the pre-configured "MEV Burn Indexer Dashboard"

**Available Metrics:**
- Transaction processing rates (successful/failed)
- Stream connection status and reconnections
- Processing time percentiles (p50, p95, p99)
- Database operation latencies
- Application uptime and health
- Balance changes recorded

The metrics endpoint is available at http://localhost:9090/metrics in Prometheus format.

### Structured Logging

The application uses structured logging with the `tracing` framework. Log output includes:

- Connection status and reconnection attempts
- Transaction processing counts (every 10 transactions)
- Parse errors (logged but don't stop the stream)
- Database operation results

Adjust the log level via the `LOG_LEVEL` environment variable:
- `trace`: Maximum verbosity (includes all spans)
- `debug`: Detailed debugging information
- `info`: General operational messages (recommended)
- `warn`: Only warnings and errors
- `error`: Only error messages

## Troubleshooting

**Connection Issues**

If you see repeated gRPC connection errors:
1. Verify your `GRPC_TOKEN` is correct
2. Check that `GRPC_ENDPOINT` is accessible from your network
3. Ensure your firewall allows outbound HTTPS connections

**Database Errors**

If migrations fail or database operations error:
1. Confirm your `DATABASE_URL` is correctly formatted
2. Verify the database user has CREATE and INSERT permissions
3. Check that the database exists and is accessible

**No Transactions Appearing**

If the stream connects but no transactions are saved:
1. Verify `TARGET_ACCOUNT` is correctly formatted (base58 address)
2. Check that the account is actually making transactions
3. Review logs for parse errors that might indicate a data format issue

## Development

### Running Tests

```bash
cargo test
```

### Code Formatting

```bash
cargo fmt
```

### Linting

```bash
cargo clippy
```

## Future Enhancements

The architecture supports several planned extensions:

1. **Instruction-Level Analysis**: Add a table to store program invocations and log messages for deeper transaction analysis
2. **PnL Calculation**: Aggregate balance changes over time to estimate profit and loss
3. **Metrics Export**: Expose Prometheus metrics for monitoring dashboards
4. **REST API**: Provide HTTP endpoints for querying historical data
5. **Multi-Account Tracking**: Extend to monitor multiple accounts simultaneously

## License

[Specify your license here]

## Contributing

[Add contribution guidelines if applicable]
