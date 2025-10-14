use thiserror::Error;

/// Application-level errors with context-rich messages.
/// 
/// All fallible operations in this application return Result<T, AppError>.
/// This enum provides specific error variants for different failure modes,
/// enabling proper error handling and informative error messages.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("gRPC connection error: {0}")]
    GrpcConnection(String),

    #[error("gRPC stream error: {0}")]
    GrpcStream(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Transaction parsing error: {0}")]
    ParseError(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Solana client error: {0}")]
    SolanaClient(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convert anyhow::Error to AppError for broader compatibility
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Config(err.to_string())
    }
}
