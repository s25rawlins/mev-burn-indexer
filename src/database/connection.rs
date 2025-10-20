use crate::error::AppError;
use refinery::embed_migrations;
use tokio_postgres::Client;
use tokio_postgres_rustls::MakeRustlsConnect;
use tracing::info;

// Embed migration files at compile time from the migrations directory
embed_migrations!("migrations");

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

/// Run database migrations using refinery.
/// 
/// This automatically applies all migration files from the /migrations directory
/// that haven't been applied yet. Refinery tracks which migrations have run
/// in a special `refinery_schema_history` table, ensuring each migration is
/// applied exactly once.
pub async fn run_migrations(client: &mut Client) -> Result<(), AppError> {
    info!("Running database migrations");

    migrations::runner()
        .run_async(client)
        .await
        .map_err(|e| AppError::Database(format!("Migration failed: {}", e)))?;

    info!("Database migrations completed successfully");

    Ok(())
}
