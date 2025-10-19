# Deployment Guide

This guide walks you through deploying the MEV Burn Indexer with its complete monitoring stack.

## Prerequisites

Before you begin, ensure you have:

- Docker and Docker Compose installed (version 3.8 or higher)
- Access to the NeonDB PostgreSQL database (connection details in `.env`)
- The gRPC endpoint credentials configured
- At least 2GB of available disk space for monitoring data

## Quick start

If you're familiar with Docker Compose and just want to get started, run these commands:

```bash
# Start the monitoring stack
docker-compose -f docker-compose.monitoring.yml up -d

# Start the indexer application
cargo run --release
```

Access the dashboards at:
- Grafana: http://localhost:3000 (admin/admin)
- Prometheus: http://localhost:9091
- Metrics endpoint: http://localhost:9091/metrics (or next available port)

## Detailed deployment steps

### Step 1: Configure environment variables

The application reads configuration from environment variables. Create a `.env` file if you don't have one:

```bash
cp .env.example .env
```

Edit `.env` and verify these settings:

```env
GRPC_ENDPOINT=https://temporal.rpcpool.com
GRPC_TOKEN=your-token-here
TARGET_ACCOUNT=MEViEnscUm6tsQRoGd9h6nLQaQspKj7DB2M5FwM3Xvz
DATABASE_URL=your-neondb-connection-string
LOG_LEVEL=info
METRICS_PORT=9090
INCLUDE_FAILED_TRANSACTIONS=true
```

### Step 2: Start the monitoring stack

The monitoring stack includes Prometheus for metrics collection and Grafana for visualization.

```bash
docker-compose -f docker-compose.monitoring.yml up -d
```

This command starts two containers:
- **Prometheus** on port 9091, scraping metrics every 15 seconds
- **Grafana** on port 3000 with pre-configured dashboards

Wait about 30 seconds for Grafana to fully initialize, then verify the containers are running:

```bash
docker-compose -f docker-compose.monitoring.yml ps
```

You should see both containers in the "Up" state.

### Step 3: Access Grafana

Open your browser and navigate to http://localhost:3000.

Login credentials:
- Username: `admin`
- Password: `admin`

You'll be prompted to change the password on first login. You can skip this if you're running locally.

### Step 3.1: Create additional user (optional)

If you need to create an additional user with specific credentials, run the provisioning script:

```bash
./monitoring/scripts/create_grafana_user.sh
```

This script creates a user with the following credentials:
- Email: `srawlins@gmail.com`
- Password: `2501`

The script automatically:
- Waits for Grafana to be fully ready
- Checks if the user already exists
- Creates the user if needed or updates the password if they exist
- Grants admin permissions to the user

You can then log in with either the default admin account or the provisioned user account.

### Step 4: Verify dashboard configuration

After logging in, navigate to **Dashboards** from the left sidebar. You should see two dashboards:

1. **MEV Burn Indexer Dashboard**: System metrics showing transaction processing rates, database performance, and stream connection status
2. **MEV Burn Analysis Dashboard**: Business metrics showing burn data, transaction volumes, and bot activity

If the PostgreSQL datasource shows connection errors, wait a few minutes for the indexer application to populate initial data.

### Step 5: Start the indexer application

With monitoring running, start the main application:

```bash
cargo run --release
```

The application will:
1. Connect to the NeonDB database
2. Run migrations if needed
3. Connect to the Yellowstone gRPC stream
4. Start processing transactions
5. Expose metrics on port 9091 (or next available)

Watch the logs for confirmation messages:
```
INFO mev_burn_indexer: Configuration loaded
INFO mev_burn_indexer: Configured to capture both successful and failed transactions
INFO mev_burn_indexer::metrics_server: Metrics server listening on port 9091
INFO mev_burn_indexer::grpc::stream_handler: Processing transaction updates
```

### Step 6: Monitor the application

Return to Grafana and open the **MEV Burn Indexer Dashboard**. You should see metrics updating in real-time:

- **Stream Status**: Should show "Connected" (green)
- **Total Transactions Processed**: Incrementing counter
- **Transaction Processing Rate**: Real-time processing throughput
- **Database Operation Time**: Performance metrics for database writes

Switch to the **MEV Burn Analysis Dashboard** to view business metrics:

- **Total Burn**: Cumulative SOL burned in lamports
- **Average Transaction Fee**: Mean fee per transaction
- **Burn Over Time**: Time series chart showing burn trends
- **Transaction Volume**: Hourly breakdown of successful and failed transactions
- **Recent Transactions**: Live table of the latest 50 transactions

## Stopping the system

To stop the application, press `Ctrl+C` in the terminal where it's running.

To stop the monitoring stack:

```bash
docker-compose -f docker-compose.monitoring.yml down
```

To stop and remove all data (including historical metrics):

```bash
docker-compose -f docker-compose.monitoring.yml down -v
```

## Troubleshooting

### Metrics server port conflict

If port 9090 is already in use, the application automatically tries alternate ports (9091, 9092, etc.). Check the logs to see which port was bound:

```
INFO mev_burn_indexer::metrics_server: Requested port was in use, bound to alternate port
```

Update the Prometheus configuration in `monitoring/prometheus.yml` if needed:

```yaml
scrape_configs:
  - job_name: 'mev-burn-indexer'
    static_configs:
      - targets: ['host.docker.internal:9091']  # Update port here
```

### Database connection errors

If you see database connection errors, verify:

1. Your DATABASE_URL is correct in `.env`
2. Your IP address is whitelisted in NeonDB
3. The database exists and is accessible

Test the connection manually:

```bash
psql "your-database-url-here" -c "SELECT 1;"
```

### Grafana shows "No data"

If dashboards show no data:

1. Wait 1-2 minutes for initial data collection
2. Verify the indexer application is running
3. Check that Prometheus is scraping metrics: http://localhost:9091/targets
4. Verify the metrics endpoint is accessible: `curl http://localhost:9091/metrics`

### Stream connection failures

If the gRPC stream fails to connect:

1. Verify GRPC_ENDPOINT and GRPC_TOKEN in `.env`
2. Check your internet connection
3. Confirm the RPC service is operational
4. Review application logs for specific error messages

The application includes automatic reconnection with exponential backoff, so temporary network issues should resolve automatically.

## Production deployment considerations

When deploying to production:

1. **Change Grafana credentials**: Update `GF_SECURITY_ADMIN_PASSWORD` in `docker-compose.monitoring.yml`
2. **Enable HTTPS**: Configure a reverse proxy (nginx, Caddy) with SSL certificates
3. **Set retention policies**: Adjust Prometheus retention in the docker-compose file (`--storage.tsdb.retention.time=30d`)
4. **Configure backups**: Set up automated backups for Grafana dashboards and Prometheus data
5. **Monitor disk usage**: Prometheus data grows over time, monitor `/var/lib/docker/volumes/`
6. **Secure the metrics endpoint**: Add authentication or restrict access to localhost only
7. **Use environment-specific configs**: Create separate `.env` files for dev, staging, and production

## Data retention

The monitoring stack retains data as follows:

- **Prometheus**: 30 days (configurable)
- **Grafana**: Dashboards and settings persist in the `grafana-data` volume
- **PostgreSQL**: All transaction data is retained indefinitely

To back up historical data:

```bash
# Backup Prometheus data
docker run --rm -v mev-burn-indexer_prometheus-data:/data -v $(pwd):/backup alpine tar czf /backup/prometheus-backup.tar.gz /data

# Backup Grafana data
docker run --rm -v mev-burn-indexer_grafana-data:/data -v $(pwd):/backup alpine tar czf /backup/grafana-backup.tar.gz /data
```

## Next steps

With the system running, you can:

- Create custom Grafana dashboards for specific metrics
- Set up alerting rules in Prometheus for critical conditions
- Export data from PostgreSQL for offline analysis
- Integrate with other monitoring tools via the metrics endpoint
