# Monitoring Setup Guide

This guide explains how to set up and use the Grafana + Prometheus monitoring stack for the MEV Burn Indexer.

## Overview

The monitoring stack consists of:
- **Prometheus**: Time-series database that scrapes metrics from your application
- **Grafana**: Visualization platform with pre-configured dashboards
- **Metrics Exporter**: Built into the application, exposes metrics on port 9090

## Quick Start

### 1. Build and Start Your Application

First, make sure you have the Rust dependencies installed:

```bash
cargo build --release
```

Start your application:

```bash
./start.sh
```

The application will automatically:
- Start the metrics server on port 9090
- Expose metrics at `http://localhost:9090/metrics`
- Expose health check at `http://localhost:9090/health`

### 2. Start the Monitoring Stack

Start Prometheus and Grafana using Docker Compose:

```bash
docker-compose -f docker-compose.monitoring.yml up -d
```

This will start:
- **Prometheus** on port 9091 (http://localhost:9091)
- **Grafana** on port 3000 (http://localhost:3000)

### 3. Access Grafana

1. Open your browser and navigate to http://localhost:3000
2. Login with default credentials:
   - Username: `admin`
   - Password: `admin`
3. You'll be prompted to change the password (recommended for production)
4. The "MEV Burn Indexer Dashboard" will be automatically available

## Available Metrics

### Transaction Metrics
- `solana_tracker_transactions_processed_total` - Total transactions successfully processed
- `solana_tracker_transactions_failed_total` - Total transactions that failed to process
- `solana_tracker_balance_changes_recorded_total` - Total balance changes recorded

### Stream Health Metrics
- `solana_tracker_stream_connected` - Stream connection status (1=connected, 0=disconnected)
- `solana_tracker_stream_reconnections_total` - Total number of reconnection attempts

### Performance Metrics
- `solana_tracker_transaction_processing_seconds` - Histogram of transaction processing times
- `solana_tracker_database_operation_seconds` - Histogram of database operation times

### Application Health
- `solana_tracker_uptime_seconds` - Application uptime in seconds
- `solana_tracker_last_transaction_timestamp` - Unix timestamp of last processed transaction
- `solana_tracker_database_connections_active` - Number of active database connections
- `solana_tracker_errors_total` - Total number of errors

## Dashboard Features

The pre-configured Grafana dashboard includes:

### Overview Panels
1. **Total Transactions Processed** - Cumulative count
2. **Stream Status** - Real-time connection status (green=connected, red=disconnected)
3. **Stream Reconnections** - Number of reconnection attempts
4. **Application Uptime** - How long the app has been running

### Performance Graphs
5. **Transaction Processing Rate** - Transactions per second (successful vs failed)
6. **Balance Changes Rate** - Balance changes per second
7. **Transaction Processing Time** - p50, p95, p99 percentiles
8. **Database Operation Time** - p50, p95, p99 percentiles

All panels update every 10 seconds by default.

## Manual Verification

### Check if Metrics are Available

```bash
curl http://localhost:9090/metrics
```

You should see Prometheus-formatted metrics like:
```
# HELP solana_tracker_transactions_processed_total Total number of transactions processed
# TYPE solana_tracker_transactions_processed_total counter
solana_tracker_transactions_processed_total 42
```

### Check Health Endpoint

```bash
curl http://localhost:9090/health
```

Should return: `OK`

### Check Prometheus Targets

1. Go to http://localhost:9091/targets
2. Verify that the `mev-burn-indexer` target is showing as "UP"

## Troubleshooting

### Prometheus Can't Connect to Application

**Issue**: Prometheus shows the target as "DOWN"

**Solutions**:

1. **For Linux users**: Edit `monitoring/prometheus.yml` and change:
   ```yaml
   - targets: ['host.docker.internal:9090']
   ```
   to:
   ```yaml
   - targets: ['172.17.0.1:9090']
   ```

2. **Alternative**: Add `--network="host"` to the Prometheus container in `docker-compose.monitoring.yml`

3. **Verify the application is running**: 
   ```bash
   curl http://localhost:9090/metrics
   ```

### Grafana Shows No Data

1. **Check Prometheus is scraping**: Go to http://localhost:9091/targets
2. **Verify data in Prometheus**: Go to http://localhost:9091/graph and query `solana_tracker_transactions_processed_total`
3. **Check time range**: In Grafana, make sure you're looking at "Last 1 hour" or a recent time range
4. **Verify datasource**: In Grafana, go to Configuration → Data Sources and ensure Prometheus is configured correctly

### Dashboard Not Loading

1. Check that dashboard file exists: `monitoring/grafana/dashboards/mev-burn-indexer.json`
2. Restart Grafana:
   ```bash
   docker-compose -f docker-compose.monitoring.yml restart grafana
   ```
3. Check Grafana logs:
   ```bash
   docker logs mev-burn-indexer-grafana
   ```

## Advanced Configuration

### Change Metrics Port

If port 9090 conflicts with another service, modify in `src/main.rs`:

```rust
let metrics_port = 9090;  // Change to your preferred port
```

Then update `monitoring/prometheus.yml` to match:

```yaml
- targets: ['host.docker.internal:YOUR_PORT']
```

### Add Custom Metrics

1. Define new metrics in `src/metrics.rs`:
   ```rust
   pub static ref MY_CUSTOM_METRIC: IntCounter = IntCounter::new(
       "solana_tracker_my_custom_metric_total",
       "Description of my metric"
   ).expect("metric can be created");
   ```

2. Register in `init_metrics()` function

3. Use in your code:
   ```rust
   metrics::MY_CUSTOM_METRIC.inc();
   ```

4. Create a new panel in Grafana to visualize it

### Alerting

You can set up alerts in Grafana:

1. Go to Alerting → Alert Rules
2. Click "New alert rule"
3. Set conditions, e.g., "Alert if stream_connected == 0 for more than 5 minutes"
4. Configure notification channels (email, Slack, etc.)

### Data Retention

By default, Prometheus keeps 30 days of data. To change:

Edit `docker-compose.monitoring.yml`:
```yaml
- '--storage.tsdb.retention.time=90d'  # Keep 90 days
```

## Stopping the Monitoring Stack

```bash
docker-compose -f docker-compose.monitoring.yml down
```

To also remove the stored data:

```bash
docker-compose -f docker-compose.monitoring.yml down -v
```

## Production Recommendations

1. **Change Grafana credentials** immediately after first login
2. **Set up HTTPS** for both Prometheus and Grafana
3. **Configure authentication** on Prometheus (it's open by default)
4. **Set up backup** for Prometheus data and Grafana dashboards
5. **Configure alerting** for critical metrics
6. **Use external storage** for long-term metrics retention
7. **Restrict network access** to monitoring ports

## Resources

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Prometheus Rust Client](https://docs.rs/prometheus/latest/prometheus/)
