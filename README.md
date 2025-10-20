# MEV Burn Indexer

A production-grade data pipeline for monitoring and analyzing trading bot activity on the Solana blockchain. This application streams real-time transaction data from the `MEViEnscUm6tsQRoGd9h6nLQaQspKj7DB2M5FwM3Xvz` account, recording transaction fees (burn), balance changes, and operational metrics to provide competitive intelligence insights.

## What this application does

The MEV Burn Indexer tracks every transaction initiated by a specific Solana trading bot, capturing:

- Transaction signatures and metadata
- Transaction fees paid (the "burn" cost)
- Success and failure status
- Computational resources consumed
- Account balance changes for both SOL and SPL tokens

All data is stored in PostgreSQL with proper indexing for efficient analysis. The application includes a complete monitoring stack with Prometheus metrics and Grafana dashboards for real-time visibility into bot operations and system health.

## Prerequisites

You'll need the following before getting started:

- **Docker and Docker Compose** (version 3.8 or later) for containerized deployment
- **Rust** 1.70 or later for local development ([install Rust](https://rustup.rs/))
- **PostgreSQL database** with create table permissions (NeonDB or local instance)
- **gRPC credentials** for temporal.rpcpool.com (provided in assignment)

## Quick start with Docker

The fastest way to run the complete stack is using Docker Compose:

```bash
# Clone the repository
git clone <repository-url>
cd mev-burn-indexer

# Create environment configuration
cp .env.example .env
# Edit .env with your database credentials and RPC tokens

# Start the complete stack (indexer, Prometheus, Grafana)
docker-compose up -d

# View logs
docker-compose logs -f indexer
```

Access the dashboards:
- Grafana: http://localhost:3000 (admin/admin)
- Prometheus: http://localhost:9091
- Metrics endpoint: http://localhost:9090/metrics

## Local development setup

For development or if you prefer running the application directly:

### Configure environment variables

Create a `.env` file with your configuration:

```bash
cp .env.example .env
```

Edit the file to include your credentials:

```env
GRPC_ENDPOINT=https://temporal.rpcpool.com
GRPC_TOKEN=your-token-here
TARGET_ACCOUNT=MEViEnscUm6tsQRoGd9h6nLQaQspKj7DB2M5FwM3Xvz
DATABASE_URL=postgresql://user:password@host:port/database?sslmode=require
LOG_LEVEL=info
```

### Build and run

```bash
# Build the application
cargo build --release

# Run the application
cargo run --release

# Or use the startup script
./start.sh
```

The application will automatically:
1. Connect to your PostgreSQL database
2. Apply database migrations using refinery (from the `migrations/` directory)
3. Connect to the Yellowstone gRPC stream
4. Start processing transactions
5. Expose metrics on port 9090 (or next available port)

The migration system uses refinery to track which schema changes have been applied. On first run, it creates the database schema. On subsequent runs, it applies only new migrations, making upgrades seamless.

### Start monitoring services

If running locally, you can still use the monitoring stack:

```bash
# Start Prometheus and Grafana
docker-compose -f docker-compose.monitoring.yml up -d
```

## Architecture overview

The application follows a modular architecture with clear separation of concerns:

**Configuration** (`src/config.rs`)
Loads and validates environment variables, providing type-safe access to application settings.

**Database layer** (`src/database/`)
- `connection.rs`: Manages PostgreSQL connections with TLS encryption
- `repository.rs`: Implements the repository pattern for all database operations

**gRPC client** (`src/grpc/`)
- `client.rs`: Establishes and maintains Yellowstone gRPC connections
- `stream_handler.rs`: Processes the account stream with automatic reconnection

**Solana parser** (`src/solana/`)
- `models.rs`: Domain models for transactions and balance changes
- `parser.rs`: Converts raw Solana transaction data into structured formats

**Error handling** (`src/error.rs`)
Custom error types using `thiserror` for precise failure context.

**Observability** (`src/telemetry.rs`, `src/metrics.rs`)
Structured logging and Prometheus metrics for monitoring system health.

### Data flow

```
gRPC Stream → Parse Transaction → Extract Metadata → Save to Database
     ↓              ↓                    ↓                  ↓
  Account      Signature          Fees, Balances      PostgreSQL
  Updates      Slot, Time         Success Status      (Indexed)
```

The application maintains a persistent connection to the gRPC stream. When a transaction occurs, it's parsed and immediately persisted. If the connection drops, exponential backoff retry logic automatically reconnects.

## Database schema

**transactions table**
Stores core transaction metadata with the following key columns:
- `signature`: Unique transaction identifier (VARCHAR(88))
- `slot`: Solana slot number for ordering (BIGINT)
- `block_time`: Transaction timestamp (TIMESTAMPTZ)
- `fee`: Transaction fee in lamports (BIGINT)
- `fee_payer`: Account that paid the fee (VARCHAR(44))
- `success`: Whether the transaction succeeded (BOOLEAN)
- `compute_units_consumed`: Computational resources used (BIGINT)

Indexes on signature (unique), slot, block_time, and fee_payer enable efficient queries.

**account_balance_changes table**
Records balance modifications for each transaction:
- `transaction_id`: Foreign key to transactions table
- `account_address`: Public key of affected account
- `mint_address`: SPL token mint (NULL for SOL)
- `pre_balance`, `post_balance`: Balances before and after (BIGINT)
- `balance_delta`: Precomputed change for aggregation queries

## Monitoring and dashboards

The application includes comprehensive monitoring capabilities:

### Prometheus metrics

- `solana_tracker_transactions_processed_total`: Cumulative transactions processed
- `solana_tracker_transactions_failed_total`: Cumulative processing failures
- `solana_tracker_stream_connected`: Connection status (1=connected, 0=disconnected)
- `solana_tracker_stream_reconnections_total`: Number of reconnection attempts
- `solana_tracker_transaction_processing_seconds`: Processing time histogram
- `solana_tracker_database_operation_seconds`: Database operation latency
- `solana_tracker_uptime_seconds`: Application uptime

### Grafana dashboards

Two pre-configured dashboards are included:

**MEV Burn Indexer Dashboard**
System metrics showing transaction processing rates, database performance, stream health, and application uptime.

**MEV Burn Analysis Dashboard**
Business metrics displaying total burn, transaction volumes, success rates, and recent transaction history.

## Usage examples

### Query total fees by day

```sql
SELECT
    DATE(block_time) as date,
    COUNT(*) as tx_count,
    SUM(fee) / 1e9 as sol_burned
FROM transactions
GROUP BY DATE(block_time)
ORDER BY date DESC;
```

### Find largest balance changes

```sql
SELECT
    t.signature,
    t.block_time,
    bc.account_address,
    bc.balance_delta / 1e9 as sol_change
FROM account_balance_changes bc
JOIN transactions t ON bc.transaction_id = t.id
WHERE bc.mint_address IS NULL
ORDER BY ABS(bc.balance_delta) DESC
LIMIT 10;
```

### Calculate success rate

```sql
SELECT
    COUNT(*) FILTER (WHERE success = true) as successful,
    COUNT(*) FILTER (WHERE success = false) as failed,
    ROUND(100.0 * COUNT(*) FILTER (WHERE success = true) / COUNT(*), 2) as success_rate
FROM transactions;
```

## Troubleshooting

### Application won't start

**Check environment configuration**
Verify your `.env` file contains all required variables with valid credentials.

**Test database connectivity**
```bash
psql "your-database-url-here" -c "SELECT 1;"
```

### Metrics server port conflicts

If port 9090 is in use, the application automatically binds to the next available port (9091, 9092, etc.). Check the startup logs to see which port was selected:

```
INFO mev_burn_indexer::metrics_server: Requested port was in use, bound to alternate port
```

Update your Prometheus configuration to match the actual port.

### Grafana shows no data

Wait 1 to 2 minutes for initial data collection, then verify:

1. Application is running: `docker-compose ps` or check process list
2. Prometheus is scraping: http://localhost:9091/targets
3. Metrics endpoint is accessible: `curl http://localhost:9090/metrics`

### Stream connection failures

The application includes automatic reconnection with exponential backoff. If you see repeated connection errors:

1. Verify `GRPC_ENDPOINT` and `GRPC_TOKEN` in `.env`
2. Check internet connectivity
3. Confirm RPC service is operational
4. Review logs for specific error messages

## Development commands

### Run tests
```bash
cargo test
```

### Format code
```bash
cargo fmt
```

### Run linter
```bash
cargo clippy
```

### View application logs
```bash
# Docker deployment
docker-compose logs -f indexer

# Local deployment
tail -f indexer.log
```

## Production deployment

For production environments, consider these enhancements:

1. **Change default passwords**: Update Grafana credentials in `docker-compose.yml`
2. **Enable HTTPS**: Configure a reverse proxy (nginx, Caddy) with SSL certificates
3. **Set up backups**: Automate PostgreSQL backups and Grafana dashboard exports
4. **Configure alerting**: Set up Prometheus alert rules for critical conditions
5. **Monitor disk usage**: Prometheus data grows over time, ensure adequate storage
6. **Restrict network access**: Limit metrics endpoint access to monitoring systems only

## Project structure

```
mev-burn-indexer/
├── src/
│   ├── main.rs              # Application entry point
│   ├── config.rs            # Configuration management
│   ├── error.rs             # Error types
│   ├── telemetry.rs         # Logging setup
│   ├── metrics.rs           # Prometheus metrics
│   ├── metrics_server.rs    # HTTP metrics endpoint
│   ├── database/            # Database layer
│   ├── grpc/                # gRPC client and stream handling
│   └── solana/              # Solana-specific models and parsers
├── monitoring/
│   ├── prometheus.yml       # Prometheus configuration (local)
│   ├── prometheus-docker.yml # Prometheus configuration (Docker)
│   └── grafana/             # Grafana dashboards and provisioning
├── docs/
│   ├── ARCHITECTURE.md      # Detailed architecture documentation
│   ├── DEPLOYMENT.md        # Deployment guide
│   └── MONITORING.md        # Monitoring setup guide
├── Dockerfile               # Container image definition
├── docker-compose.yml       # Full stack orchestration
└── .env.example             # Environment template
```

## Additional resources

- [Detailed architecture documentation](docs/ARCHITECTURE.md)
- [Deployment guide](docs/DEPLOYMENT.md)
- [Monitoring setup guide](docs/MONITORING.md)
- [Yellowstone gRPC documentation](https://docs.triton.one/project-yellowstone/dragons-mouth-grpc-subscriptions)
- [Solana JSON-RPC documentation](https://solana.com/docs/rpc)
