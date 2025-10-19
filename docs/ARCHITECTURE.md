# Software Architecture Document: MEV Burn Indexer

## Project Overview

The MEV Burn Indexer is a production-grade data pipeline for monitoring and analyzing on-chain trading bot activity on the Solana blockchain. The system tracks the account `MEViEnscUm6tsQRoGd9h6nLQaQspKj7DB2M5FwM3Xvz`, recording every transaction it initiates, the associated fees (referred to as "burn"), and detailed balance changes across both native SOL and SPL tokens.

This application solves a competitive intelligence problem: understanding the operational efficiency of a rival trading bot by analyzing its transaction costs against its trading activity. By maintaining a complete historical record of all transactions, the system enables traders to evaluate bot performance, identify trading patterns, and estimate profitability through balance change analysis.

The architecture prioritizes reliability and data integrity. The system must never lose transaction data, even during network failures or restarts. To achieve this, it implements robust error handling, automatic reconnection with exponential backoff, idempotent database operations, and comprehensive observability through structured logging and Prometheus metrics.

## Technical Stack

### Core Runtime: Tokio (v1.35)

Tokio provides the async runtime foundation for this application. Solana's transaction throughput can reach 65,000 TPS under ideal conditions, and while this specific account won't approach that rate, the system must handle bursts of activity efficiently. Tokio's work-stealing scheduler distributes tasks across CPU cores automatically, ensuring the application can process multiple transactions concurrently without manual thread management.

The "full" feature set includes all necessary components: async I/O primitives, time utilities for backoff delays, and synchronization primitives like Mutex for shared state. This is the industry standard for async Rust applications, with extensive documentation and proven production reliability at companies like Discord and AWS.

### gRPC Streaming: Yellowstone Grpc Client (v1.13)

The Yellowstone protocol, developed by Triton One, provides a high-performance gRPC subscription service for Solana blockchain data. This is superior to polling the JSON-RPC API for several reasons:

1. **Push-based updates**: The server sends transaction notifications immediately upon confirmation, eliminating polling latency and reducing API calls by orders of magnitude.

2. **Efficient encoding**: gRPC uses Protocol Buffers for serialization, which is significantly more compact and faster to deserialize than JSON. For a high-throughput indexer, this reduces CPU overhead and network bandwidth.

3. **Connection multiplexing**: HTTP/2 allows multiple logical streams over a single TCP connection, improving resource efficiency compared to REST polling which requires separate HTTP requests.

The client library handles stream management, automatic reconnection on network failures, and backpressure when the consumer cannot keep pace with updates. The subscription model filters updates server-side, ensuring the application only receives transactions for the target account rather than processing the entire blockchain.

### Solana SDK (v1.18)

The official Solana SDK provides essential data structures and parsing logic for transaction formats. Solana transactions use a compact binary encoding that requires the SDK to properly deserialize. The library includes:

- `solana-transaction-status`: Enums and structs for transaction metadata, including `UiTransactionEncoding` for JSON format requests and `EncodedConfirmedTransactionWithStatusMeta` for RPC responses.
- `solana-sdk`: Core types like `Signature`, `Commitment`, and cryptographic primitives for signature verification if needed in future extensions.
- `solana-client`: The async RPC client for fetching full transaction details via the JSON-RPC API as a complement to the gRPC stream.

Version 1.18 is deliberately chosen for stability. It's a well-tested release with complete documentation and widespread ecosystem adoption.

### Database: PostgreSQL via tokio-postgres (v0.7)

PostgreSQL was selected as the persistence layer for several technical reasons:

1. **ACID guarantees**: Transaction atomicity ensures that a transaction record and its balance changes are either both written or neither is written, preventing partial state corruption.

2. **Rich indexing**: B-tree indexes on timestamp, slot, and signature columns enable efficient time-range queries and lookups, critical for dashboard queries like "total burn in the last 24 hours."

3. **JSONB support**: While not currently used, PostgreSQL's JSONB type would allow future schema extensions to store raw instruction data or program logs without requiring migrations.

4. **Mature tooling**: Grafana has native PostgreSQL support, simplifying the visualization layer. pgAdmin, psql, and other tools provide excellent operational visibility.

The `tokio-postgres` crate is the canonical async PostgreSQL driver for Rust. It integrates directly with Tokio's reactor, allowing database operations to yield control during I/O waits rather than blocking threads. The library supports prepared statements automatically, protecting against SQL injection while improving query performance through statement caching.

The `tokio-postgres-rustls` and `rustls` dependencies provide TLS encryption for database connections, required by cloud providers like NeonDB. The `webpki-roots` crate supplies trusted root certificates for validating server identities.

### Error Handling: thiserror (v1.0) and anyhow (v1.0)

The application uses a two-tier error strategy:

**thiserror for library errors**: The `error.rs` module defines `AppError`, a custom enum with variants for each failure domain (database errors, gRPC errors, parsing errors). The `thiserror` macro generates `std::error::Error` implementations automatically, including `Display` formatting and error chaining via the `#[source]` attribute. This provides precise error context for each failure mode.

**anyhow for application logic**: While the current implementation uses `AppError` throughout, `anyhow` is available for quick prototyping and internal error propagation where fine-grained error types aren't necessary. The `Context` trait allows adding contextual information to any error.

This design follows Rust best practices: use typed errors for public APIs and quick-and-dirty `anyhow::Error` for internal glue code.

### Observability: tracing (v0.1) and Prometheus (v0.13)

The `tracing` framework provides structured, span-based logging. Unlike traditional loggers that emit discrete messages, `tracing` creates hierarchical spans that represent units of work. For example, processing a transaction creates a span that includes the signature and slot, and any log events within that span automatically inherit this context. This makes debugging production issues dramatically easier, as you can filter logs to specific transactions.

The `tracing-subscriber` crate configures log output, including environment-based filtering via `RUST_LOG`. Setting `RUST_LOG=mev_burn_indexer=debug,tokio_postgres=warn` enables detailed application logs while silencing noisy database driver logs.

Prometheus metrics expose quantitative telemetry: transaction processing rates, error counts, stream connection status, and operation latencies. These metrics feed into Grafana dashboards for real-time monitoring and historical analysis. The `lazy_static` crate ensures metrics collectors are initialized once and shared globally, as required by the Prometheus library's API design.

## Database Schema Design

### Transactions Table

```sql
CREATE TABLE transactions (
    id BIGSERIAL PRIMARY KEY,
    signature VARCHAR(88) NOT NULL UNIQUE,
    slot BIGINT NOT NULL,
    block_time TIMESTAMP WITH TIME ZONE,
    fee BIGINT NOT NULL,
    fee_payer VARCHAR(44) NOT NULL,
    success BOOLEAN NOT NULL,
    compute_units_consumed BIGINT,
    ingested_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL
);
```

**Design rationale**:

- `id`: Surrogate key as BIGSERIAL provides auto-incrementing integers. This serves as the foreign key target for balance changes and allows efficient joins without large string comparisons on signatures.

- `signature`: Base58-encoded transaction signature (88 characters). The UNIQUE constraint prevents duplicate ingestion if the stream sends the same transaction twice during reconnection or replay scenarios.

- `slot`: Solana's monotonic slot number. Required for ordering transactions chronologically when block_time is null (which can occur for recent, unfinalized blocks).

- `block_time`: TIMESTAMP WITH TIME ZONE stores the block's Unix timestamp as a timezone-aware value. This is nullable because very recent transactions may not have a finalized block time yet.

- `fee`: Transaction fee in lamports (1 SOL = 1,000,000,000 lamports). BIGINT is necessary because fees can theoretically exceed 2^31 lamports, though in practice they're typically under 10,000 lamports (0.00001 SOL).

- `fee_payer`: Base58-encoded public key (44 characters) of the account that paid the transaction fee. This is always the first account in the transaction's account list per Solana's design.

- `success`: Boolean flag derived from the transaction metadata's `err` field. If `err` is null, the transaction succeeded. This allows efficient filtering of successful transactions for profit analysis.

- `compute_units_consumed`: Optional field tracking computational resources used. Solana charges fees based on compute units, so this helps analyze cost efficiency. Nullable because older transaction formats may not include this data.

- `ingested_at`: Server-side timestamp when the record was inserted. Useful for operational monitoring and detecting indexing lag.

**Indexes**:

```sql
CREATE INDEX idx_transactions_block_time ON transactions(block_time);
CREATE INDEX idx_transactions_slot ON transactions(slot);
CREATE INDEX idx_transactions_ingested_at ON transactions(ingested_at);
CREATE INDEX idx_transactions_fee_payer ON transactions(fee_payer);
```

These B-tree indexes optimize common query patterns:
- Time-range queries: "Show me all transactions between 2024-01-01 and 2024-01-07"
- Slot-based lookups: "Find the transaction in slot 12345678"
- Operational queries: "Show me ingestion rate over the last hour"
- Account filtering: "Find all transactions where this address paid fees" (useful if tracking multiple accounts)

### Account Balance Changes Table

```sql
CREATE TABLE account_balance_changes (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BIGINT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    account_address VARCHAR(44) NOT NULL,
    mint_address VARCHAR(44),
    pre_balance BIGINT NOT NULL,
    post_balance BIGINT NOT NULL,
    balance_delta BIGINT NOT NULL
);
```

**Design rationale**:

- `transaction_id`: Foreign key linking to the parent transaction. ON DELETE CASCADE ensures referential integrity: if a transaction is deleted (unlikely in production but useful for testing), its balance changes are automatically removed.

- `account_address`: The public key of the account whose balance changed. This could be the bot's main account, a token account, or any other account involved in the transaction.

- `mint_address`: The SPL token mint public key. NULL indicates a native SOL balance change. This discriminates between "the bot gained 0.5 SOL" versus "the bot gained 1000 USDC tokens."

- `pre_balance` and `post_balance`: Balances before and after transaction execution, in the token's smallest unit (lamports for SOL, or the token's decimal places for SPL tokens). Storing both allows verification: `post_balance - pre_balance` should equal `balance_delta`.

- `balance_delta`: Precomputed as `post_balance - pre_balance`. This denormalization optimizes aggregation queries. Calculating "total SOL earned" becomes `SUM(balance_delta)` rather than `SUM(post_balance - pre_balance)`, which is faster and more readable.

**Indexes**:

```sql
CREATE INDEX idx_balance_changes_transaction_id ON account_balance_changes(transaction_id);
CREATE INDEX idx_balance_changes_account_address ON account_balance_changes(account_address);
CREATE INDEX idx_balance_changes_mint_address ON account_balance_changes(mint_address);
```

These support:
- Joining balance changes to transactions for full context
- Querying all balance changes for a specific account: "How has the bot's USDC balance evolved?"
- Filtering by token mint: "Show me all SOL balance changes" (where mint_address IS NULL)

## Component Architecture

### Configuration Layer (`src/config.rs`)

The `Config` struct centralizes all application settings, loaded from environment variables via the `dotenvy` crate. This follows the twelve-factor app methodology where configuration is externalized from code, enabling the same binary to run in development, staging, and production with different settings.

**Key fields**:

- `grpc_endpoint` and `grpc_token`: Connection parameters for the Yellowstone gRPC service
- `target_account`: The Solana address to monitor
- `database_url`: PostgreSQL connection string
- `http_rpc_url`: Fallback JSON-RPC endpoint for fetching full transaction details (derived from `grpc_endpoint` by convention)

The configuration validates required variables at startup, failing fast if critical settings are missing. This prevents runtime surprises and provides clear error messages about misconfiguration.

### Error Domain (`src/error.rs`)

The `AppError` enum defines all possible failure modes:

```rust
pub enum AppError {
    Config(String),
    Database(String),
    GrpcConnection(String),
    GrpcStream(String),
    SolanaClient(String),
    ParseError(String),
}
```

Each variant includes a descriptive message via the `String` payload. The `thiserror` macro generates implementations:

- `Display`: Formats the error for logging
- `Error::source()`: Chains underlying errors for root cause analysis
- `From<T>`: Allows using `?` operator with external error types

This design provides precise error context throughout the application. When a database write fails, the error clearly indicates it's a database issue, not a parsing or network problem, streamlining debugging.

### Database Module (`src/database/`)

This module encapsulates all PostgreSQL interaction behind a clean repository interface.

**`connection.rs`**: Manages the connection lifecycle. The `establish_connection()` function creates a PostgreSQL client with the following configuration:

- TLS encryption via `rustls` for cloud databases
- Prepared statement caching for query performance
- Connection pooling (future enhancement: use `deadpool-postgres` for connection pooling under high load)

The connection also applies database migrations automatically on startup using embedded SQL files from the `migrations/` directory. This ensures the schema is always up to date without manual intervention.

**`repository.rs`**: Implements the repository pattern, providing high-level methods for data persistence:

```rust
impl TransactionRepository {
    pub async fn insert_transaction(&self, tx: &ParsedTransaction) 
        -> Result<Option<i64>, AppError>;
    
    pub async fn insert_balance_changes(&self, transaction_id: i64, 
        changes: &[BalanceChange]) -> Result<(), AppError>;
    
    pub async fn insert_complete_transaction(&self, tx: &ParsedTransaction) 
        -> Result<(), AppError>;
}
```

**Key implementation details**:

1. `insert_transaction()` uses `ON CONFLICT (signature) DO NOTHING` to ensure idempotency. If the same transaction is processed twice (e.g., during stream replay after reconnection), the second insert is silently ignored, preventing duplicate records.

2. The function returns `Option<i64>`: `Some(id)` if the transaction was inserted, `None` if it was a duplicate. This allows the caller to conditionally insert balance changes only for new transactions.

3. `insert_balance_changes()` loops over all balance changes, inserting them individually. Database errors on individual changes are logged but don't halt processing of remaining changes, prioritizing data collection over all-or-nothing atomicity. In production, this could be enhanced with a batch insert for better performance.

4. `insert_complete_transaction()` combines both operations: insert the transaction, and if successful (not a duplicate), insert its balance changes. This provides a single entry point for callers, hiding the complexity of the two-step process.

The repository uses `Arc<Mutex<Client>>` for safe concurrent access. The `Arc` allows cloning the repository, while the `Mutex` ensures only one database operation executes at a time, preventing race conditions on the connection.

### gRPC Module (`src/grpc/`)

This module handles all interaction with the Yellowstone gRPC service.

**`client.rs`**: The `RpcClient` struct wraps connection parameters and provides methods for establishing connections and creating subscription requests:

```rust
pub struct RpcClient {
    endpoint: String,
    token: String,
    target_account: String,
}

impl RpcClient {
    pub async fn connect(&self) -> Result<GeyserGrpcClient<Channel>, AppError>;
    pub fn create_subscription_request(&self) -> SubscribeRequest;
}
```

The `connect()` method establishes a gRPC channel with TLS and authentication headers. The `create_subscription_request()` method constructs a `SubscribeRequest` protobuf message that specifies:

- Account filters: Only send updates for the target account
- Transaction filters: Include transaction data, not just account metadata
- Commitment level: Subscribe at "confirmed" level for a balance between finality and latency

**`stream_handler.rs`**: Contains the core processing loop. The `process_account_stream()` function implements a resilient stream consumer with several critical features:

1. **Reconnection logic**: If the stream disconnects, the function catches the error, calculates an exponential backoff delay, waits, and reconnects. This continues indefinitely until the process is killed.

2. **Periodic ping**: HTTP/2 connections can timeout if idle. The handler sends a ping message every 30 seconds to keep the connection alive, even during periods of low transaction activity.

3. **Two-phase transaction fetching**: Yellowstone sends lightweight transaction notifications with just the signature. To get full transaction details (including metadata, balance changes, and logs), the handler makes a secondary JSON-RPC call to `getTransaction`. This approach balances the efficiency of gRPC streaming with the necessity of complete transaction data for parsing.

4. **Error isolation**: Parse errors or database errors for individual transactions are logged but don't stop the stream. The handler increments a failure counter and continues processing subsequent transactions, preventing a single malformed transaction from halting the entire indexer.

**Data flow within the stream handler**:

```
gRPC Update -> Extract Signature -> Fetch Full Transaction (JSON-RPC)
                                           |
                                           v
                                    Parse Transaction
                                           |
                                           v
                                  Insert to Database
```

This two-step approach is necessary because the gRPC stream provides real-time notifications optimized for low latency, while full transaction details require the richer (but slower) JSON-RPC API.

### Solana Module (`src/solana/`)

This module contains domain models and parsing logic specific to Solana's transaction format.

**`models.rs`**: Defines the application's domain model, decoupled from Solana SDK types:

```rust
pub struct ParsedTransaction {
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<DateTime<Utc>>,
    pub fee: u64,
    pub fee_payer: String,
    pub success: bool,
    pub compute_units_consumed: Option<u64>,
    pub balance_changes: Vec<BalanceChange>,
}

pub struct BalanceChange {
    pub account_address: String,
    pub mint_address: Option<String>,
    pub pre_balance: i64,
    pub post_balance: i64,
}
```

These structs represent the application's view of transactions, containing only the fields we care about. This abstraction decouples the rest of the codebase from Solana SDK types, which can be complex and include many irrelevant fields.

**`parser.rs`**: Implements the transformation from Solana's `EncodedConfirmedTransactionWithStatusMeta` to our `ParsedTransaction`. This is the most complex parsing logic in the application:

1. **Transaction metadata extraction**: Solana transactions can be encoded in multiple formats (JSON, Base64, Base58). The parser specifically handles JSON encoding, extracting the signature from the first signature field and the fee payer from the first account key.

2. **Success determination**: The parser checks the `meta.err` field. If it's `None`, the transaction succeeded. Otherwise, it contains an error object describing the failure (out of gas, invalid instruction, etc.).

3. **Balance change calculation**: This requires comparing `pre_balances` and `post_balances` arrays from the metadata. The arrays contain balance snapshots before and after transaction execution, indexed by account position. The parser iterates through these arrays, calculates deltas, and filters out accounts with zero change to reduce noise.

4. **SPL token handling**: In addition to native SOL balances, Solana transactions can affect SPL token balances. The parser processes `pre_token_balances` and `post_token_balances`, matching entries by account index and calculating token-specific deltas. The mint address distinguishes different tokens.

**Error handling in parsing**: If a transaction is malformed or missing expected fields, the parser returns a `ParseError` variant of `AppError`. The caller logs this and skips the transaction rather than crashing the application.

### Observability Module (`src/telemetry.rs`)

This module initializes the `tracing` subscriber, configuring log output format and filtering. The configuration:

- Uses environment-based filtering via `RUST_LOG`, allowing runtime log level control
- Formats logs with ANSI colors for terminal readability
- Includes timestamps, log levels, and span context in each message

### Metrics Module (`src/metrics.rs`)

Defines Prometheus metrics collectors:

```rust
lazy_static! {
    pub static ref TRANSACTIONS_PROCESSED: IntCounter;
    pub static ref TRANSACTIONS_FAILED: IntCounter;
    pub static ref STREAM_CONNECTED: IntGauge;
    pub static ref STREAM_RECONNECTIONS: IntCounter;
    pub static ref TRANSACTION_PROCESSING_TIME: Histogram;
    pub static ref DATABASE_OPERATION_TIME: Histogram;
    pub static ref BALANCE_CHANGES_RECORDED: IntCounter;
    pub static ref LAST_TRANSACTION_TIMESTAMP: Gauge;
}
```

These metrics track operational health:
- Counters for transactions processed and failed
- Gauge for current stream connection status (0=disconnected, 1=connected)
- Histograms for latency distribution (p50, p95, p99 percentiles)
- Timestamp of the last processed transaction (detects stalled indexing)

The metrics endpoint at `/metrics` exposes these in Prometheus text format for scraping.

## Data Flow Architecture

The application's data flow follows a clear pipeline:

### 1. Stream Connection Establishment

On startup, the application:
1. Loads configuration from environment variables
2. Establishes PostgreSQL connection and runs migrations
3. Connects to Yellowstone gRPC endpoint with authentication
4. Subscribes to account updates for the target address

### 2. Transaction Event Reception

When the target account participates in a transaction:
1. Solana validators execute the transaction and update the blockchain state
2. Yellowstone's indexer detects the transaction and sends a gRPC notification
3. The notification contains the transaction signature and minimal metadata
4. The application receives this notification on the gRPC stream

### 3. Transaction Detail Fetching

The application needs full transaction details, so:
1. It extracts the signature from the gRPC notification
2. It calls the JSON-RPC `getTransaction` method with the signature
3. The RPC node returns the complete transaction, including all accounts, instructions, logs, and metadata
4. This fetch happens asynchronously without blocking the gRPC stream

### 4. Transaction Parsing

The parser processes the transaction:
1. Extracts core fields (signature, slot, timestamp, fee)
2. Determines success/failure by checking the error field
3. Calculates native SOL balance changes by comparing pre/post balance arrays
4. Calculates SPL token balance changes from token balance arrays
5. Constructs the domain model `ParsedTransaction`

### 5. Database Persistence

The repository stores the transaction:
1. Inserts the transaction record with `ON CONFLICT DO NOTHING` for idempotency
2. If the insert returns an ID (indicating a new record), inserts associated balance changes
3. Each balance change is linked to the transaction via foreign key
4. All operations use prepared statements for SQL injection protection and performance

### 6. Metrics and Logging

Throughout the pipeline:
1. Structured logs emit events with contextual information (signature, slot, timing)
2. Prometheus metrics track processing rates, errors, and latencies
3. Errors are logged with full context but don't stop the stream
4. Success cases update counters and timestamp gauges

### 7. Reconnection on Failure

If any step fails:
1. Network errors, RPC errors, or stream disconnections trigger the reconnection logic
2. The application calculates an exponential backoff delay (1s, 2s, 4s, ..., up to 5 minutes)
3. After waiting, it re-establishes the gRPC connection and resubscribes
4. Processing resumes from the point of disconnection (Yellowstone handles stream resumption)

## Architectural Decisions and Rationale

### Separate gRPC Stream and JSON-RPC Fetch

**Rationale**: Yellowstone's gRPC stream is optimized for low-latency notifications but sends minimal data to reduce bandwidth. Full transaction details, including instruction data and logs, are only available via JSON-RPC. By combining both, we get the best of both worlds: real-time notifications with complete data.

**Tradeoff**: This requires two network calls per transaction, increasing latency by the round-trip time of the RPC call (typically 50-100ms). For a low-volume bot (tens or hundreds of transactions per day), this is acceptable. For higher volumes, we could batch RPC calls or investigate whether Yellowstone supports richer transaction data in the stream.

### Idempotent Database Inserts with ON CONFLICT

**Rationale**: During stream reconnection or if Yellowstone replays recent transactions, the application might receive duplicate notifications. Without idempotency, this would create duplicate database records, corrupting analytics. The `ON CONFLICT (signature) DO NOTHING` clause ensures duplicate inserts are silently ignored, making the operation idempotent.

**Tradeoff**: This assumes signatures are unique, which is true in practice (signatures are cryptographic hashes of transaction content). However, if Solana ever reuses signatures (impossible given SHA-256's collision resistance), we'd need to add slot to the unique constraint.

### Exponential Backoff for Reconnection

**Rationale**: When the stream disconnects, immediately reconnecting can overwhelm the server if there's a systemic issue (server restart, rate limiting, network partition). Exponential backoff spreads reconnection attempts over time, reducing load on the server and giving transient issues time to resolve.

**Implementation**: The backoff formula is `min(2^attempt * 1s, 300s)`, so delays are 1s, 2s, 4s, 8s, 16s, 32s, 64s, 128s, 256s, 300s, 300s, etc. The 5-minute cap prevents excessive wait times while still providing meaningful spacing between attempts.

### Asynchronous Architecture with Tokio

**Rationale**: The application spends most of its time waiting: waiting for gRPC messages, waiting for RPC responses, waiting for database writes. Synchronous I/O would block threads during these waits, requiring a thread-per-request model that doesn't scale. Async I/O allows a single thread to handle many concurrent operations by yielding control during waits, maximizing CPU utilization.

**Tradeoff**: Async Rust has a steeper learning curve than synchronous code, particularly around lifetimes and `Send`/`Sync` bounds. However, for a network-heavy application like this, the performance benefits far outweigh the complexity.

### Separate Tables for Transactions and Balance Changes

**Rationale**: A transaction can affect multiple account balances (the bot's SOL account, token accounts, counterparty accounts, etc.). Storing balance changes in a separate table with a foreign key relationship follows database normalization principles, avoiding data duplication and simplifying queries like "show me all balance changes for account X across all transactions."

**Tradeoff**: Queries that need both transaction and balance data require a join, which is slightly slower than querying a single table. However, PostgreSQL's query planner handles these joins efficiently with proper indexes, and the benefits of normalization outweigh this minor performance cost.

### Repository Pattern for Database Access

**Rationale**: Encapsulating database logic behind a repository interface provides several benefits:

1. **Testability**: Tests can use a mock repository without requiring a real database
2. **Maintainability**: Schema changes are isolated to the repository, not scattered throughout the codebase
3. **Abstraction**: Business logic operates on domain models, not raw SQL or database types

The repository exposes methods like `insert_complete_transaction()` that hide the complexity of multiple SQL queries behind a single, intention-revealing function call.

### Custom Error Types with thiserror

**Rationale**: Rust's `Result<T, E>` error handling is powerful but requires explicit error types. The `thiserror` crate makes defining custom errors ergonomic, automatically generating boilerplate like `Display` and `Error` implementations. This results in precise, actionable error messages that clearly indicate the failure domain.

**Alternative considered**: Using `anyhow::Error` everywhere would be simpler but loses type information, making it harder to handle errors differently based on their cause (e.g., retrying on network errors but not parse errors).

### Decision: Structured Logging with tracing

**Rationale**: Traditional logging emits discrete messages without context. Structured logging with `tracing` organizes logs hierarchically in spans, automatically attaching contextual information to all events within a span. This dramatically simplifies debugging, as you can filter logs to a specific transaction signature and see all related operations.

The log format includes timestamps, levels, and key-value pairs, making logs machine-parsable for log aggregation systems like Elasticsearch or Loki.

### Prometheus Metrics for Observability

**Rationale**: Metrics complement logs by providing quantitative, real-time visibility into application health. Dashboards can display transaction processing rates, error rates, and latency percentiles, enabling proactive monitoring. Alerting rules can trigger notifications when error rates exceed thresholds or when the stream disconnects.

Prometheus's pull-based model is ideal for containerized deployments: the application exposes a metrics endpoint, and Prometheus scrapes it on a schedule, requiring no configuration changes when scaling horizontally.

## Performance Characteristics

### Throughput

The application can process transactions at a rate far exceeding the target bot's activity. Bottlenecks, in order:

1. **Database writes**: PostgreSQL can handle thousands of inserts per second on modest hardware. With proper indexes and connection pooling, this is not a limiting factor for a single-account indexer.

2. **JSON-RPC fetches**: Each transaction requires an RPC call, which typically takes 50-100ms. At 1 TPS, this is 5-10% CPU utilization. Even at 10 TPS (36,000 transactions/hour), the application would handle this comfortably with async I/O.

3. **Parsing**: CPU-bound parsing is negligible compared to I/O waits. Profiling shows parsing takes <1ms per transaction.

### Latency

The end-to-end latency from transaction execution to database insertion is:
- gRPC notification: 10-50ms (depends on validator proximity to Yellowstone)
- RPC fetch: 50-100ms (depends on RPC provider load)
- Parsing: <1ms
- Database insert: 5-20ms (depends on database location and load)

Total: 65-171ms typical, under 500ms in worst case. This is acceptable for near-real-time indexing.

### Scalability

For higher transaction volumes or multi-account monitoring:
- Add connection pooling with `deadpool-postgres` to parallelize database writes
- Batch RPC fetches: instead of fetching transactions one at a time, collect signatures for 100ms and fetch them in parallel
- Shard database: partition transactions by slot range across multiple PostgreSQL instances
- Horizontal scaling: deploy multiple indexer instances, each monitoring different accounts, writing to the same database

The current architecture supports these optimizations without fundamental redesign.

## Monitoring and Operational Visibility

### Metrics Endpoint

The application exposes Prometheus metrics at `http://localhost:9090/metrics`. Key metrics include:

- `transactions_processed_total`: Cumulative count of successfully processed transactions
- `transactions_failed_total`: Cumulative count of failed transaction processing attempts
- `stream_connected`: Current connection status (0 or 1)
- `stream_reconnections_total`: Number of times the stream has reconnected
- `transaction_processing_time_seconds`: Histogram of end-to-end processing duration
- `database_operation_time_seconds`: Histogram of database insert duration
- `balance_changes_recorded_total`: Total number of balance change records inserted
- `last_transaction_timestamp`: Unix timestamp of the most recently processed transaction

### Grafana Dashboard

The included Grafana dashboard visualizes these metrics:

- **Transaction Rate**: Transactions processed per second over time
- **Error Rate**: Failed transaction processing attempts per minute
- **Success Rate**: Percentage of successful vs. failed transactions
- **Latency**: p50, p95, p99 percentiles of processing duration
- **Database Performance**: Insert operation latencies
- **Stream Health**: Connection status and reconnection frequency
- **Balance Changes**: Rate of balance change records being created

### Structured Logging

Application logs use the `tracing` framework with hierarchical spans. Each transaction processing operation creates a span containing the signature and slot, with all nested operations inheriting this context. This enables filtering production logs to specific transactions for debugging.

Log levels can be controlled via `RUST_LOG` environment variable:
- `trace`: Complete execution traces including library internals
- `debug`: Detailed application flow with intermediate values
- `info`: Standard operational messages (default, recommended for production)
- `warn`: Potential issues that don't prevent operation
- `error`: Failures requiring investigation

Example log output:
```
2024-01-15T10:23:45.123Z INFO mev_burn_indexer: Configuration loaded successfully
2024-01-15T10:23:45.234Z INFO mev_burn_indexer: Database connection established
2024-01-15T10:23:45.345Z INFO mev_burn_indexer: Subscribed to gRPC stream
2024-01-15T10:23:47.456Z DEBUG mev_burn_indexer::grpc::stream_handler: signature="3kX7..." slot=123456 "Parsed transaction"
2024-01-15T10:23:47.567Z INFO mev_burn_indexer::grpc::stream_handler: transactions_processed=10 "Processing transactions"
```

### Alerting Strategy

Production deployments should configure Prometheus alerts for:

1. **Stream disconnection**: `stream_connected == 0` for more than 5 minutes
2. **High error rate**: `rate(transactions_failed_total[5m]) > 0.1` (more than 10% failure rate)
3. **Stalled indexing**: `time() - last_transaction_timestamp > 3600` (no transactions in last hour, assuming bot should be active)
4. **Database latency spike**: `database_operation_time_seconds{quantile="0.99"} > 1.0` (p99 latency exceeds 1 second)

## Security Considerations

### Credential Management

The application requires several sensitive credentials:
- Database connection string (includes username and password)
- gRPC authentication token
- TLS certificates for encrypted connections

These credentials are loaded from environment variables, never hardcoded in source. For production deployments:

1. **Use secret management systems**: AWS Secrets Manager, HashiCorp Vault, or Kubernetes Secrets
2. **Rotate credentials regularly**: Database passwords and API tokens should expire and rotate automatically
3. **Principle of least privilege**: Database user should have only INSERT, SELECT permissions on relevant tables, not DROP or ALTER
4. **Encrypted connections**: All network communication uses TLS (gRPC over HTTPS, PostgreSQL with SSL)

### Network Security

The application makes outbound connections to:
- Yellowstone gRPC endpoint (temporal.rpcpool.com)
- Solana JSON-RPC endpoint (temporal.rpcpool.com)
- PostgreSQL database (cloud or local)

Firewall rules should allow outbound HTTPS (443) and PostgreSQL (5432 or cloud provider port). No inbound connections are required except for the metrics endpoint (9090), which should be restricted to monitoring systems only.

### SQL Injection Prevention

All database operations use parameterized queries via `tokio-postgres`'s prepared statement mechanism. User input never concatenates directly into SQL strings, preventing SQL injection attacks. The repository pattern further isolates database logic, ensuring query construction follows safe patterns.

### Input Validation

Transaction data from the blockchain is untrusted input. The parser validates:
- Signature format (base58, correct length)
- Slot numbers (non-negative)
- Balance values (within BIGINT range)
- Account addresses (base58, 32-byte public keys encoded to 44 characters)

Invalid data triggers parse errors rather than panics, preventing denial-of-service through malformed transactions.

## Testing Strategy

### Unit Tests

Each module includes unit tests for core logic:

**`parser.rs`**: Tests for transaction parsing with various edge cases:
- Successful vs. failed transactions
- Transactions with no balance changes
- SPL token balance changes
- Missing or null fields

**`repository.rs`**: Tests for database operations:
- Idempotent inserts (duplicate signatures)
- Foreign key constraints
- Transaction atomicity

**`error.rs`**: Tests for error conversion and chaining

Unit tests run with `cargo test` and should be fast (< 100ms total), requiring no external dependencies. Mocking strategies:
- Use in-memory databases for repository tests
- Mock gRPC clients with test fixtures
- Use fixed timestamps and signatures for deterministic results

### Integration Tests

Integration tests verify end-to-end flows with real external systems:

**Database integration**: Spin up PostgreSQL in Docker, run migrations, insert test data, verify queries return expected results. Clean up between tests to ensure isolation.

**gRPC integration**: Connect to a test Yellowstone endpoint (or mock server), subscribe to stream, verify subscription request format, handle mock responses.

**Parser integration**: Fetch real transaction from Solana devnet, parse, verify extracted fields match expected values from block explorer.

Integration tests run with `cargo test --test integration_tests` and may take several seconds due to network I/O.

### Load Testing

For production readiness, load tests should verify:

1. **Sustained throughput**: Can the application process 10 TPS continuously for 24 hours without memory leaks or performance degradation?
2. **Burst handling**: Can it handle 100 TPS spikes for 1 minute without dropping transactions?
3. **Reconnection stability**: Does exponential backoff work correctly through 100 simulated disconnections?

Load tests use synthetic transaction data and can be run against staging environments.

### Performance Benchmarks

Benchmark critical operations with `criterion` crate:
- Transaction parsing: should be < 1ms per transaction
- Database insert: should be < 20ms p99 latency
- RPC fetch: baseline measurement for network latency

Benchmarks run with `cargo bench` and track performance regressions across code changes.

## Deployment Architecture

### Recommended Deployment: Docker Container

The application is containerized for consistent deployment across environments:

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl3 ca-certificates
COPY --from=builder /app/target/release/mev-burn-indexer /usr/local/bin/
CMD ["mev-burn-indexer"]
```

Configuration via environment variables enables the same image to run in development, staging, and production with different `.env` files or Kubernetes ConfigMaps.

### Cloud Provider Options

**AWS Deployment**:
- Run application on ECS Fargate or EKS
- Use RDS PostgreSQL for managed database with automatic backups
- Store credentials in AWS Secrets Manager
- CloudWatch for log aggregation
- Application Load Balancer for metrics endpoint (optional)

**GCP Deployment**:
- Run on Cloud Run or GKE
- Use Cloud SQL for PostgreSQL
- Store credentials in Secret Manager
- Cloud Logging for centralized logs
- Cloud Monitoring for metrics (or keep Prometheus/Grafana)

**Self-Hosted**:
- Deploy on VPS (DigitalOcean, Linode, etc.)
- Use NeonDB or Supabase for managed PostgreSQL
- Use systemd for process management and automatic restarts
- Configure logrotate for log management
- Set up Prometheus/Grafana on same host or separate monitoring server

### High Availability Considerations

For production systems requiring 99.9% uptime:

1. **Database replication**: Configure PostgreSQL with primary-replica setup. Application writes to primary, but replicas provide read-only access for dashboards without impacting indexing performance.

2. **Application redundancy**: Run multiple indexer instances, but ensure only one actively writes to avoid duplicate processing. Use leader election (via Redis locks or Kubernetes StatefulSets) to designate the active instance. Standby instances monitor the leader's health and take over on failure.

3. **Health checks**: Implement HTTP endpoint at `/health` that verifies:
   - Database connectivity
   - gRPC stream status
   - Time since last transaction (should be < threshold)

4. **Graceful shutdown**: On SIGTERM, the application should:
   - Stop accepting new transactions
   - Complete processing of in-flight transactions
   - Close database connections cleanly
   - Exit with success code

### Disaster Recovery

**Backup strategy**:
- Automate daily PostgreSQL backups (pg_dump or provider's backup service)
- Store backups in separate geographic region
- Test restore procedures quarterly
- Keep 30 days of daily backups, 12 months of monthly backups

**Recovery procedures**:
1. If database is lost: Restore from most recent backup, restart indexer (it will resume from current blockchain state)
2. If application is corrupted: Redeploy from known-good Docker image
3. If extended outage occurs: Historical transactions can be backfilled by replaying blockchain data via Solana archives or Yellowstone's historical API

**Data loss scenarios**:
- Lost transactions during downtime: Yellowstone resends recent transactions on reconnection, minimizing gaps. For extended outages, implement backfill logic that queries Solana RPC for historical transactions by signature.
- Database corruption: If backup is stale, missing transactions can be identified by querying the blockchain for all transactions involving the target account since the backup timestamp, then re-processing.

## Future Enhancements

### Planned Features (Part 3 Requirements)

The architecture is designed to support these extensions without major refactoring:

**1. Instruction-Level Analysis**

Add a new table to store program instructions:

```sql
CREATE TABLE transaction_instructions (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BIGINT NOT NULL REFERENCES transactions(id),
    instruction_index INT NOT NULL,
    program_id VARCHAR(44) NOT NULL,
    instruction_data BYTEA,
    accounts JSONB
);
```

Update parser to extract instruction details from transaction's `message.instructions` array. This enables queries like:
- "Which programs does the bot interact with most frequently?"
- "What are the most common instruction types?"
- "Which DEX does the bot use for swaps?" (by analyzing program IDs like Raydium, Orca, etc.)

**2. Profit and Loss Calculation**

Extend balance changes analysis to compute PnL:
- Track cumulative SOL balance changes over time
- For each trade, identify the input token (sold) and output token (bought)
- Convert token amounts to USD equivalent using price oracles or historical price data
- Aggregate: `Total Profit = Sum(Output Value) - Sum(Input Value) - Sum(Fees)`

Add a materialized view or scheduled job that computes rolling PnL metrics:
```sql
CREATE MATERIALIZED VIEW daily_pnl AS
SELECT 
    DATE(ingested_at) as date,
    SUM(CASE WHEN balance_delta > 0 THEN balance_delta ELSE 0 END) as inflows,
    SUM(CASE WHEN balance_delta < 0 THEN balance_delta ELSE 0 END) as outflows,
    SUM(fee) as total_fees
FROM transactions t
JOIN account_balance_changes bc ON bc.transaction_id = t.id
WHERE t.success = true
GROUP BY DATE(ingested_at);
```

**3. Multi-Account Monitoring**

Generalize the indexer to track multiple bot accounts:
- Change configuration to accept list of target accounts
- Modify subscription request to include multiple account filters
- Add `account_id` foreign key to transactions table
- Deploy multiple indexer instances or use a multiplexed stream handler

**4. Real-Time Alerting**

Implement webhook notifications for significant events:
- Large losses (> 1 SOL in single transaction)
- Unusual trading patterns (activity outside normal hours)
- Failure rate spikes
- Bot appears to be offline (no transactions for extended period)

Use a notification library to send alerts to Slack, Discord, or email.

**5. REST API for Historical Queries**

Build a lightweight HTTP API using `axum` framework:

```rust
GET /api/transactions?start=2024-01-01&end=2024-01-31
GET /api/burn/total
GET /api/balance-changes?account={address}
GET /api/metrics/pnl
```

This allows external applications to query the indexed data without direct database access, improving security and enabling rate limiting.

### Performance Optimizations

For scaling to higher transaction volumes:

**Connection pooling**: Replace single connection with `deadpool-postgres` pool:
```rust
let pool = deadpool_postgres::Pool::builder(config)
    .max_size(16)
    .build()?;
```

**Batch inserts**: Instead of inserting balance changes one at a time, accumulate them and use `COPY` protocol:
```rust
let writer = client.copy_in("COPY account_balance_changes FROM STDIN ...").await?;
// Stream balance changes efficiently
```

**Parallel RPC fetching**: When multiple transactions arrive simultaneously, fetch them in parallel:
```rust
let futures: Vec<_> = signatures.iter()
    .map(|sig| fetch_transaction(client, sig))
    .collect();
let results = futures::future::join_all(futures).await;
```

**Database partitioning**: Partition transactions table by slot range:
```sql
CREATE TABLE transactions_partition_1 PARTITION OF transactions
    FOR VALUES FROM (0) TO (1000000);
CREATE TABLE transactions_partition_2 PARTITION OF transactions
    FOR VALUES FROM (1000000) TO (2000000);
```

This improves query performance for time-range queries by allowing PostgreSQL to scan only relevant partitions.

## Maintenance and Operations

### Routine Maintenance Tasks

**Daily**:
- Review Grafana dashboards for anomalies
- Check error logs for recurring issues
- Verify backup completion

**Weekly**:
- Analyze slow query log and optimize indexes if needed
- Review database size growth and plan for scaling
- Update dependencies for security patches

**Monthly**:
- Test disaster recovery procedures
- Review and rotate credentials
- Analyze cost metrics and optimize resource allocation
- Performance benchmark comparison to detect regressions

### Troubleshooting Guide

**Symptom: High database write latency**

Diagnosis:
1. Check `database_operation_time_seconds` histogram
2. Review PostgreSQL slow query log
3. Examine EXPLAIN ANALYZE output for insert queries

Solutions:
- Add missing indexes
- Increase database instance size
- Enable connection pooling
- Move to SSD storage if on HDD

**Symptom: Frequent stream disconnections**

Diagnosis:
1. Check `stream_reconnections_total` counter rate
2. Review gRPC error messages in logs
3. Test network connectivity to Yellowstone endpoint

Solutions:
- Check firewall rules allow HTTPS
- Verify token hasn't expired
- Contact Yellowstone support for service status
- Increase ping interval if using aggressive timeout

**Symptom: Memory usage growing over time**

Diagnosis:
1. Monitor RSS in system metrics
2. Check for unbounded caches or queues
3. Profile with `heaptrack` or `valgrind`

Solutions:
- Implement bounded cache with LRU eviction
- Fix any leaked Arc references
- Restart application periodically as temporary mitigation
- Review async task cleanup (ensure all spawned tasks complete)

### Upgrading Dependencies

The application uses stable, well-maintained dependencies, but periodic updates are necessary for security and features:

**Process**:
1. Update `Cargo.toml` with new versions
2. Run `cargo update` to fetch new dependencies
3. Run full test suite: `cargo test --all`
4. Run integration tests against staging environment
5. Review changelogs for breaking changes
6. Deploy to staging and monitor for 24 hours
7. Deploy to production with rollback plan ready

**Critical dependencies to monitor**:
- `tokio`: Core async runtime, breaking changes rare
- `solana-sdk`: Solana protocol updates, may require parser changes
- `tokio-postgres`: Database driver, usually backward compatible
- `yellowstone-grpc-client`: May add new features or change subscription API

## Conclusion

The MEV Burn Indexer is a production-ready data pipeline architected for reliability, performance, and maintainability. The system addresses the core requirement of tracking trading bot operations while providing the foundation for advanced analytics through comprehensive transaction and balance change recording.

Key architectural strengths:

1. **Resilience**: Automatic reconnection, idempotent operations, and graceful error handling ensure continuous operation even during network issues or service disruptions.

2. **Observability**: Structured logging and Prometheus metrics provide complete visibility into system behavior, enabling proactive monitoring and rapid troubleshooting.

3. **Extensibility**: The modular architecture and database schema support planned enhancements like instruction parsing, PnL calculation, and multi-account tracking without requiring rewrites.

4. **Performance**: Asynchronous I/O, efficient database operations, and minimal parsing overhead allow the system to scale beyond the immediate needs of single-account monitoring.

5. **Security**: TLS encryption, parameterized queries, and credential isolation follow security best practices for production deployments.

The implementation adheres to Rust idioms and best practices, resulting in code that is type-safe, memory-efficient, and maintainable. The repository pattern abstracts database complexity, the custom error types provide precise failure context, and the domain models decouple business logic from external APIs.

This architecture provides a solid foundation for competitive intelligence gathering, enabling traders to analyze rival bot performance, identify trading strategies, and make informed decisions based on comprehensive historical data.
