use crate::error::AppError;
use tokio_postgres::Client;
use tokio_postgres_rustls::MakeRustlsConnect;
use tracing::info;

/// Create a PostgreSQL client connection.
/// 
/// This establishes a connection to PostgreSQL using tokio-postgres.
/// The connection is managed manually since tokio-postgres doesn't have
/// a built-in connection pool like sqlx.
pub async fn create_client(database_url: &str) -> Result<Client, AppError> {
    info!("Establishing database connection");

    // Create TLS connector for secure database connections (required for Neon and other cloud providers)
    let mut root_store = rustls::RootCertStore::empty();
    root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
        rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));
    
    let tls_config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    
    let tls_connector = MakeRustlsConnect::new(tls_config);

    let (client, connection) = tokio_postgres::connect(database_url, tls_connector)
        .await
        .map_err(|e| AppError::Database(format!("Failed to connect: {}", e)))?;

    // Spawn the connection to run in the background
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Database connection error: {}", e);
        }
    });

    info!("Database connection established successfully");

    Ok(client)
}

/// Run database migrations manually.
/// 
/// Since we're not using sqlx anymore, we'll execute the migration SQL directly.
pub async fn run_migrations(client: &Client) -> Result<(), AppError> {
    info!("Running database migrations");

    // Create transactions table
    client
        .execute(
            r#"
            CREATE TABLE IF NOT EXISTS transactions (
                id BIGSERIAL PRIMARY KEY,
                signature VARCHAR(88) NOT NULL UNIQUE,
                slot BIGINT NOT NULL,
                block_time TIMESTAMP WITH TIME ZONE,
                fee BIGINT NOT NULL,
                fee_payer VARCHAR(44) NOT NULL,
                success BOOLEAN NOT NULL,
                compute_units_consumed BIGINT,
                ingested_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL
            )
            "#,
            &[],
        )
        .await
        .map_err(|e| AppError::Database(format!("Failed to create transactions table: {}", e)))?;

    // Create indexes for transactions
    client
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_transactions_block_time ON transactions(block_time)",
            &[],
        )
        .await
        .map_err(|e| AppError::Database(format!("Failed to create index: {}", e)))?;

    client
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_transactions_slot ON transactions(slot)",
            &[],
        )
        .await
        .map_err(|e| AppError::Database(format!("Failed to create index: {}", e)))?;

    client
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_transactions_ingested_at ON transactions(ingested_at)",
            &[],
        )
        .await
        .map_err(|e| AppError::Database(format!("Failed to create index: {}", e)))?;

    client
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_transactions_fee_payer ON transactions(fee_payer)",
            &[],
        )
        .await
        .map_err(|e| AppError::Database(format!("Failed to create index: {}", e)))?;

    // Create balance_changes table
    client
        .execute(
            r#"
            CREATE TABLE IF NOT EXISTS account_balance_changes (
                id BIGSERIAL PRIMARY KEY,
                transaction_id BIGINT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
                account_address VARCHAR(44) NOT NULL,
                mint_address VARCHAR(44),
                pre_balance BIGINT NOT NULL,
                post_balance BIGINT NOT NULL,
                balance_delta BIGINT NOT NULL
            )
            "#,
            &[],
        )
        .await
        .map_err(|e| AppError::Database(format!("Failed to create balance_changes table: {}", e)))?;

    // Create indexes for balance_changes
    client
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_balance_changes_transaction_id ON account_balance_changes(transaction_id)",
            &[],
        )
        .await
        .map_err(|e| AppError::Database(format!("Failed to create index: {}", e)))?;

    client
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_balance_changes_account_address ON account_balance_changes(account_address)",
            &[],
        )
        .await
        .map_err(|e| AppError::Database(format!("Failed to create index: {}", e)))?;

    client
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_balance_changes_mint_address ON account_balance_changes(mint_address)",
            &[],
        )
        .await
        .map_err(|e| AppError::Database(format!("Failed to create index: {}", e)))?;

    info!("Database migrations completed successfully");

    Ok(())
}
