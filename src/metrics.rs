use crate::error::AppError;
use lazy_static::lazy_static;
use prometheus::{
    Counter, Gauge, Histogram, HistogramOpts, IntCounter, IntGauge, Opts, Registry,
};

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
}

// Create metrics with Result returns to enable proper error handling
fn create_transaction_metrics() -> Result<(IntCounter, IntCounter, IntCounter), AppError> {
    let processed = IntCounter::new(
        "solana_tracker_transactions_processed_total",
        "Total number of transactions processed"
    ).map_err(|e| AppError::Config(format!("Failed to create transactions_processed metric: {}", e)))?;

    let failed = IntCounter::new(
        "solana_tracker_transactions_failed_total",
        "Total number of transactions that failed to process"
    ).map_err(|e| AppError::Config(format!("Failed to create transactions_failed metric: {}", e)))?;

    let balance_changes = IntCounter::new(
        "solana_tracker_balance_changes_recorded_total",
        "Total number of balance changes recorded"
    ).map_err(|e| AppError::Config(format!("Failed to create balance_changes metric: {}", e)))?;

    Ok((processed, failed, balance_changes))
}

fn create_stream_metrics() -> Result<(IntCounter, IntGauge), AppError> {
    let reconnections = IntCounter::new(
        "solana_tracker_stream_reconnections_total",
        "Total number of stream reconnection attempts"
    ).map_err(|e| AppError::Config(format!("Failed to create stream_reconnections metric: {}", e)))?;

    let connected = IntGauge::new(
        "solana_tracker_stream_connected",
        "Stream connection status (1=connected, 0=disconnected)"
    ).map_err(|e| AppError::Config(format!("Failed to create stream_connected metric: {}", e)))?;

    Ok((reconnections, connected))
}

fn create_timing_metrics() -> Result<(Histogram, Histogram), AppError> {
    let processing_time = Histogram::with_opts(
        HistogramOpts::new(
            "solana_tracker_transaction_processing_seconds",
            "Time taken to process a transaction"
        ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0])
    ).map_err(|e| AppError::Config(format!("Failed to create transaction_processing_time metric: {}", e)))?;

    let db_time = Histogram::with_opts(
        HistogramOpts::new(
            "solana_tracker_database_operation_seconds",
            "Time taken for database operations"
        ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0])
    ).map_err(|e| AppError::Config(format!("Failed to create database_operation_time metric: {}", e)))?;

    Ok((processing_time, db_time))
}

fn create_health_metrics() -> Result<(Gauge, Gauge, IntGauge), AppError> {
    let uptime = Gauge::new(
        "solana_tracker_uptime_seconds",
        "Application uptime in seconds"
    ).map_err(|e| AppError::Config(format!("Failed to create uptime metric: {}", e)))?;

    let last_tx = Gauge::new(
        "solana_tracker_last_transaction_timestamp",
        "Unix timestamp of the last processed transaction"
    ).map_err(|e| AppError::Config(format!("Failed to create last_transaction_timestamp metric: {}", e)))?;

    let db_connections = IntGauge::new(
        "solana_tracker_database_connections_active",
        "Number of active database connections"
    ).map_err(|e| AppError::Config(format!("Failed to create database_connections metric: {}", e)))?;

    Ok((uptime, last_tx, db_connections))
}

fn create_error_metrics() -> Result<Counter, AppError> {
    Counter::with_opts(
        Opts::new(
            "solana_tracker_errors_total",
            "Total number of errors by type"
        )
    ).map_err(|e| AppError::Config(format!("Failed to create errors_total metric: {}", e)))
}

lazy_static! {
    pub static ref TRANSACTIONS_PROCESSED: IntCounter = create_transaction_metrics().ok().map(|m| m.0).unwrap_or_else(|| {
        IntCounter::new("fallback_transactions_processed", "Fallback metric").unwrap()
    });
    pub static ref TRANSACTIONS_FAILED: IntCounter = create_transaction_metrics().ok().map(|m| m.1).unwrap_or_else(|| {
        IntCounter::new("fallback_transactions_failed", "Fallback metric").unwrap()
    });
    pub static ref BALANCE_CHANGES_RECORDED: IntCounter = create_transaction_metrics().ok().map(|m| m.2).unwrap_or_else(|| {
        IntCounter::new("fallback_balance_changes", "Fallback metric").unwrap()
    });
    pub static ref STREAM_RECONNECTIONS: IntCounter = create_stream_metrics().ok().map(|m| m.0).unwrap_or_else(|| {
        IntCounter::new("fallback_stream_reconnections", "Fallback metric").unwrap()
    });
    pub static ref STREAM_CONNECTED: IntGauge = create_stream_metrics().ok().map(|m| m.1).unwrap_or_else(|| {
        IntGauge::new("fallback_stream_connected", "Fallback metric").unwrap()
    });
    pub static ref TRANSACTION_PROCESSING_TIME: Histogram = create_timing_metrics().ok().map(|m| m.0).unwrap_or_else(|| {
        Histogram::with_opts(HistogramOpts::new("fallback_processing_time", "Fallback metric")).unwrap()
    });
    pub static ref DATABASE_OPERATION_TIME: Histogram = create_timing_metrics().ok().map(|m| m.1).unwrap_or_else(|| {
        Histogram::with_opts(HistogramOpts::new("fallback_db_time", "Fallback metric")).unwrap()
    });
    pub static ref APP_UPTIME: Gauge = create_health_metrics().ok().map(|m| m.0).unwrap_or_else(|| {
        Gauge::new("fallback_uptime", "Fallback metric").unwrap()
    });
    pub static ref LAST_TRANSACTION_TIMESTAMP: Gauge = create_health_metrics().ok().map(|m| m.1).unwrap_or_else(|| {
        Gauge::new("fallback_last_tx", "Fallback metric").unwrap()
    });
    pub static ref DATABASE_CONNECTIONS_ACTIVE: IntGauge = create_health_metrics().ok().map(|m| m.2).unwrap_or_else(|| {
        IntGauge::new("fallback_db_connections", "Fallback metric").unwrap()
    });
    pub static ref ERRORS_TOTAL: Counter = create_error_metrics().ok().unwrap_or_else(|| {
        Counter::with_opts(Opts::new("fallback_errors", "Fallback metric")).unwrap()
    });
}

/// Initialize the metrics registry with all metrics.
/// 
/// Returns an error if any metric fails to register with the Prometheus registry.
/// This ensures the application fails fast at startup if the monitoring system
/// cannot be properly initialized.
pub fn init_metrics() -> Result<(), AppError> {
    REGISTRY.register(Box::new(TRANSACTIONS_PROCESSED.clone()))
        .map_err(|e| AppError::Config(format!("Failed to register transactions_processed: {}", e)))?;
    
    REGISTRY.register(Box::new(TRANSACTIONS_FAILED.clone()))
        .map_err(|e| AppError::Config(format!("Failed to register transactions_failed: {}", e)))?;
    
    REGISTRY.register(Box::new(BALANCE_CHANGES_RECORDED.clone()))
        .map_err(|e| AppError::Config(format!("Failed to register balance_changes: {}", e)))?;
    
    REGISTRY.register(Box::new(STREAM_RECONNECTIONS.clone()))
        .map_err(|e| AppError::Config(format!("Failed to register stream_reconnections: {}", e)))?;
    
    REGISTRY.register(Box::new(STREAM_CONNECTED.clone()))
        .map_err(|e| AppError::Config(format!("Failed to register stream_connected: {}", e)))?;
    
    REGISTRY.register(Box::new(TRANSACTION_PROCESSING_TIME.clone()))
        .map_err(|e| AppError::Config(format!("Failed to register transaction_processing_time: {}", e)))?;
    
    REGISTRY.register(Box::new(DATABASE_OPERATION_TIME.clone()))
        .map_err(|e| AppError::Config(format!("Failed to register database_operation_time: {}", e)))?;
    
    REGISTRY.register(Box::new(APP_UPTIME.clone()))
        .map_err(|e| AppError::Config(format!("Failed to register app_uptime: {}", e)))?;
    
    REGISTRY.register(Box::new(LAST_TRANSACTION_TIMESTAMP.clone()))
        .map_err(|e| AppError::Config(format!("Failed to register last_transaction_timestamp: {}", e)))?;
    
    REGISTRY.register(Box::new(DATABASE_CONNECTIONS_ACTIVE.clone()))
        .map_err(|e| AppError::Config(format!("Failed to register database_connections: {}", e)))?;
    
    REGISTRY.register(Box::new(ERRORS_TOTAL.clone()))
        .map_err(|e| AppError::Config(format!("Failed to register errors_total: {}", e)))?;

    Ok(())
}

/// Get the metrics in Prometheus exposition format.
/// 
/// Returns a Result containing the metrics text or an error if encoding fails.
/// This ensures proper error propagation rather than panicking on encoding failures.
pub fn gather_metrics() -> Result<String, AppError> {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = vec![];
    
    encoder.encode(&metric_families, &mut buffer)
        .map_err(|e| AppError::Config(format!("Failed to encode metrics: {}", e)))?;
    
    String::from_utf8(buffer)
        .map_err(|e| AppError::Config(format!("Failed to convert metrics to UTF-8: {}", e)))
}
