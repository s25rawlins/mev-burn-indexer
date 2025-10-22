# MEV Burn Indexer Deliverables

This document outlines the project deliverables and how to access them.

## 1. GitHub Repository

**Repository URL:** https://github.com/s25rawlins/mev-burn-indexer

The repository contains:
- Complete Rust source code for the indexing application
- Database migration files
- Docker Compose configuration for full stack deployment
- Grafana dashboard JSON exports
- Comprehensive documentation

To run the project, see the Quick Start section in README.md.

## 2. Database Access

### Option A: Using Your Own Database

You can run the indexer with your own PostgreSQL database:

1. Copy `.env.example` to `.env`
2. Add your database connection string to `DATABASE_URL`
3. Start the application with `docker-compose up -d`

The application will automatically create the schema and begin indexing.

### Option B: Sample Data Export

If you need sample data to review the database structure without running the indexer, you can export CSV files:

```bash
# Export sample transactions
psql $DATABASE_URL -c "COPY (SELECT * FROM transactions LIMIT 1000) TO STDOUT WITH CSV HEADER" > sample_transactions.csv

# Export sample balance changes
psql $DATABASE_URL -c "COPY (SELECT * FROM account_balance_changes LIMIT 1000) TO STDOUT WITH CSV HEADER" > sample_balance_changes.csv
```

The database schema is fully documented in:
- `migrations/20240101000000_create_transactions_table.sql`
- `migrations/20240101000001_create_balance_changes_table.sql`
- `docs/ARCHITECTURE.md` (Database Schema section)

## 3. Dashboard Access

### Grafana Dashboard Exports

The complete Grafana dashboard configurations are available as JSON exports:

**Location:**
- `monitoring/grafana/dashboards/mev-burn-dashboard.json` (Main dashboard)
- `monitoring/grafana/dashboards/mev-burn-expanded.json` (Part 3 expanded dashboard)

You can import these files into any Grafana instance.

### Running Dashboards Locally

To view the dashboards with live data:

```bash
# Start the complete stack
docker-compose up -d

# Access Grafana
# URL: http://localhost:3000
# Default credentials: admin/admin
```

Navigate to:
- "MEV Burn Analysis Dashboard" for transaction and burn metrics
- "MEV Burn Analysis, Expanded (Part 3)" for profit/loss analysis

The dashboards will auto refresh every 30 seconds with live data from your database.

### Dashboard Screenshots

If you need static views of the dashboards, take screenshots after running the stack:

1. Start the application: `docker-compose up -d`
2. Access Grafana at http://localhost:3000
3. Navigate to each dashboard
4. Capture screenshots showing the visualization panels

Recommended screenshots:
- Main dashboard showing transaction metrics
- Expanded dashboard showing PnL analysis
- Individual panel zoom views for key metrics

## Quick Verification Steps

To verify the deliverables work correctly:

1. **Clone the repository:**
   ```bash
   git clone https://github.com/s25rawlins/mev-burn-indexer.git
   cd mev-burn-indexer
   ```

2. **Configure environment:**
   ```bash
   cp .env.example .env
   # Edit .env with your database credentials
   ```

3. **Start the stack:**
   ```bash
   docker-compose up -d
   ```

4. **Verify operation:**
   ```bash
   # Check logs
   docker-compose logs -f indexer
   
   # View metrics
   curl http://localhost:9090/metrics
   
   # Access dashboards
   # Open http://localhost:3000 in browser
   ```

5. **Query the database:**
   ```bash
   psql $DATABASE_URL -c "SELECT COUNT(*) FROM transactions;"
   psql $DATABASE_URL -c "SELECT COUNT(*) FROM account_balance_changes;"
   ```

## Documentation

Complete documentation is available in the `docs/` directory:

- `README.md`: Quick start guide and usage examples
- `docs/ARCHITECTURE.md`: Detailed system architecture
- `docs/PART3_COMPLETION.md`: Part 3 implementation details
- `docs/DEPLOYMENT.md`: Deployment guide
- `docs/MONITORING.md`: Monitoring setup guide

## Support

If you encounter any issues:

1. Check the troubleshooting section in README.md
2. Review logs: `docker-compose logs -f`
3. Verify environment variables in `.env`
4. Ensure database is accessible
5. Confirm gRPC credentials are valid

## Technical Stack Summary

- **Language:** Rust 1.70+
- **Database:** PostgreSQL (NeonDB compatible)
- **Monitoring:** Prometheus + Grafana
- **Deployment:** Docker Compose
- **Data Source:** Yellowstone gRPC (Triton One)
