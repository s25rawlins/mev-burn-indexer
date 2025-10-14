use lazy_static::lazy_static;
use prometheus::{
    Counter, Gauge, Histogram, HistogramOpts, IntCounter, IntGauge, Opts, Registry,
};

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    // Transaction metrics
    pub static ref TRANSACTIONS_PROCESSED: IntCounter = IntCounter::new(
        "solana_tracker_transactions_processed_total",
        "Total number of transactions processed"
    ).expect("metric can be created");

    pub static ref TRANSACTIONS_FAILED: IntCounter = IntCounter::new(
        "solana_tracker_transactions_failed_total",
        "Total number of transactions that failed to process"
    ).expect("metric can be created");

    pub static ref BALANCE_CHANGES_RECORDED: IntCounter = IntCounter::new(
        "solana_tracker_balance_changes_recorded_total",
        "Total number of balance changes recorded"
    ).expect("metric can be created");

    // Stream health metrics
    pub static ref STREAM_RECONNECTIONS: IntCounter = IntCounter::new(
        "solana_tracker_stream_reconnections_total",
        "Total number of stream reconnection attempts"
    ).expect("metric can be created");

    pub static ref STREAM_CONNECTED: IntGauge = IntGauge::new(
        "solana_tracker_stream_connected",
        "Stream connection status (1=connected, 0=disconnected)"
    ).expect("metric can be created");

    // Processing time metrics
    pub static ref TRANSACTION_PROCESSING_TIME: Histogram = Histogram::with_opts(
        HistogramOpts::new(
            "solana_tracker_transaction_processing_seconds",
            "Time taken to process a transaction"
        ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0])
    ).expect("metric can be created");

    pub static ref DATABASE_OPERATION_TIME: Histogram = Histogram::with_opts(
        HistogramOpts::new(
            "solana_tracker_database_operation_seconds",
            "Time taken for database operations"
        ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0])
    ).expect("metric can be created");

    // Application health
    pub static ref APP_UPTIME: Gauge = Gauge::new(
        "solana_tracker_uptime_seconds",
        "Application uptime in seconds"
    ).expect("metric can be created");

    pub static ref LAST_TRANSACTION_TIMESTAMP: Gauge = Gauge::new(
        "solana_tracker_last_transaction_timestamp",
        "Unix timestamp of the last processed transaction"
    ).expect("metric can be created");

    // Database metrics
    pub static ref DATABASE_CONNECTIONS_ACTIVE: IntGauge = IntGauge::new(
        "solana_tracker_database_connections_active",
        "Number of active database connections"
    ).expect("metric can be created");

    // Error metrics
    pub static ref ERRORS_TOTAL: Counter = Counter::with_opts(
        Opts::new(
            "solana_tracker_errors_total",
            "Total number of errors by type"
        )
    ).expect("metric can be created");
}

/// Initialize the metrics registry with all metrics
pub fn init_metrics() {
    REGISTRY.register(Box::new(TRANSACTIONS_PROCESSED.clone()))
        .expect("collector can be registered");
    
    REGISTRY.register(Box::new(TRANSACTIONS_FAILED.clone()))
        .expect("collector can be registered");
    
    REGISTRY.register(Box::new(BALANCE_CHANGES_RECORDED.clone()))
        .expect("collector can be registered");
    
    REGISTRY.register(Box::new(STREAM_RECONNECTIONS.clone()))
        .expect("collector can be registered");
    
    REGISTRY.register(Box::new(STREAM_CONNECTED.clone()))
        .expect("collector can be registered");
    
    REGISTRY.register(Box::new(TRANSACTION_PROCESSING_TIME.clone()))
        .expect("collector can be registered");
    
    REGISTRY.register(Box::new(DATABASE_OPERATION_TIME.clone()))
        .expect("collector can be registered");
    
    REGISTRY.register(Box::new(APP_UPTIME.clone()))
        .expect("collector can be registered");
    
    REGISTRY.register(Box::new(LAST_TRANSACTION_TIMESTAMP.clone()))
        .expect("collector can be registered");
    
    REGISTRY.register(Box::new(DATABASE_CONNECTIONS_ACTIVE.clone()))
        .expect("collector can be registered");
    
    REGISTRY.register(Box::new(ERRORS_TOTAL.clone()))
        .expect("collector can be registered");
}

/// Get the metrics in Prometheus exposition format
pub fn gather_metrics() -> String {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
