# Docker deployment guide

This guide walks you through deploying the MEV Burn Indexer using Docker containers. The containerized deployment includes the indexer application, Prometheus for metrics collection, and Grafana for visualization, all orchestrated with Docker Compose.

## Why use Docker

Containerization provides several benefits for this application:

**Consistency across environments**
The same Docker image runs identically in development, staging, and production, eliminating "works on my machine" issues.

**Simplified dependencies**
You don't need to install Rust, manage system libraries, or configure environment paths. Docker handles all dependencies internally.

**Easy orchestration**
Docker Compose starts all services (indexer, Prometheus, Grafana) with a single command and manages their interconnections.

**Production ready**
Containers are industry-standard for production deployments, supported by all major cloud providers.

## Prerequisites

Install Docker and Docker Compose on your system:

**Ubuntu/Debian**
```bash
# Install Docker
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker $USER

# Install Docker Compose
sudo apt-get update
sudo apt-get install docker-compose-plugin

# Log out and log back in for group changes to take effect
```

**macOS**
Download and install [Docker Desktop for Mac](https://www.docker.com/products/docker-desktop/).

**Windows**
Download and install [Docker Desktop for Windows](https://www.docker.com/products/docker-desktop/).

Verify installation:
```bash
docker --version
docker-compose --version
```

## Configuration

### Create environment file

Copy the example environment file:
```bash
cp .env.example .env
```

Edit `.env` with your credentials:
```env
# gRPC streaming endpoint
GRPC_ENDPOINT=https://temporal.rpcpool.com
GRPC_TOKEN=your-grpc-token-here

# Target Solana account to monitor
TARGET_ACCOUNT=MEViEnscUm6tsQRoGd9h6nLQaQspKj7DB2M5FwM3Xvz

# PostgreSQL connection (NeonDB or other cloud provider)
DATABASE_URL=postgresql://user:password@host:port/database?sslmode=require

# Application configuration
LOG_LEVEL=info
METRICS_PORT=9090
INCLUDE_FAILED_TRANSACTIONS=true

# PostgreSQL connection details for Grafana (extracted from DATABASE_URL)
POSTGRES_HOST=your-host.neon.tech
POSTGRES_PORT=5432
POSTGRES_DB=mev-burn-indexer
POSTGRES_USER=your-username
POSTGRES_PASSWORD=your-password

# Grafana user provisioning (optional)
GRAFANA_USER_EMAIL=your-email@example.com
GRAFANA_USER_PASSWORD=your-secure-password
GRAFANA_USER_NAME=Your Full Name
GRAFANA_USER_LOGIN=your-email@example.com
```

**Database URL format:**
- NeonDB: `postgresql://user:pass@ep-xxx.neon.tech/dbname?sslmode=require`
- Local: `postgresql://user:pass@host.docker.internal:5432/dbname`
- AWS RDS: `postgresql://user:pass@xxx.rds.amazonaws.com:5432/dbname?sslmode=require`

### Update Grafana credentials (recommended)

For production deployments, change the default Grafana password.

Edit `docker-compose.yml`:
```yaml
grafana:
  environment:
    - GF_SECURITY_ADMIN_USER=admin
    - GF_SECURITY_ADMIN_PASSWORD=your-secure-password-here
```

## Building the Docker image

Build the indexer application image:

```bash
# Build from the project root directory
docker-compose build indexer
```

The build process uses a multi-stage Dockerfile that:
1. Compiles the Rust application in an optimized builder container
2. Creates a minimal runtime image with only necessary dependencies
3. Runs the application as a non-root user for security

Build time varies from 5 to 15 minutes depending on your hardware and network speed.

## Starting the application

### Start all services

```bash
# Start in detached mode (background)
docker-compose up -d

# View startup logs
docker-compose logs -f
```

This command starts three containers:
- `mev-burn-indexer`: The indexer application
- `mev-burn-indexer-prometheus`: Metrics collection
- `mev-burn-indexer-grafana`: Visualization dashboards

### Verify services are running

```bash
docker-compose ps
```

You should see all three containers in the "Up" state:
```
NAME                            STATUS
mev-burn-indexer                Up (healthy)
mev-burn-indexer-prometheus     Up
mev-burn-indexer-grafana        Up
```

### Access the dashboards

Once running, access the following URLs:

- **Grafana**: http://localhost:3000
  - Username: `admin`
  - Password: `admin` (or your custom password)

- **Prometheus**: http://localhost:9091
  - No authentication required by default

- **Metrics endpoint**: http://localhost:9090/metrics
  - Raw Prometheus format metrics

## Monitoring the application

### View logs

**All services:**
```bash
docker-compose logs -f
```

**Specific service:**
```bash
docker-compose logs -f indexer
docker-compose logs -f prometheus
docker-compose logs -f grafana
```

**Recent logs only:**
```bash
docker-compose logs --tail=50 indexer
```

### Check application health

The indexer container includes a health check that monitors the metrics endpoint:

```bash
# View health status
docker inspect mev-burn-indexer | grep -A 10 Health

# Or check via Docker Compose
docker-compose ps
```

A healthy application will show "(healthy)" in the status.

### Verify data collection

After 1 to 2 minutes of operation, verify transactions are being processed:

**Check metrics endpoint:**
```bash
curl http://localhost:9090/metrics | grep solana_tracker_transactions_processed_total
```

**Query Prometheus directly:**
```bash
curl -s "http://localhost:9091/api/v1/query?query=solana_tracker_transactions_processed_total" | jq '.data.result[0].value'
```

**Check database:**
```bash
psql "your-database-url" -c "SELECT COUNT(*) FROM transactions;"
```

## Stopping and restarting

### Stop all services

```bash
# Stop containers (preserves data)
docker-compose down

# Stop and remove volumes (deletes Prometheus/Grafana data)
docker-compose down -v
```

### Restart services

```bash
# Restart all containers
docker-compose restart

# Restart specific service
docker-compose restart indexer
```

### Update and redeploy

After making code changes:

```bash
# Rebuild the image
docker-compose build indexer

# Restart with new image
docker-compose up -d indexer
```

## Troubleshooting

### Container won't start

**View detailed error logs:**
```bash
docker-compose logs indexer
```

**Check container events:**
```bash
docker events --filter container=mev-burn-indexer
```

**Common issues:**
- Missing or invalid `.env` file
- Database connection refused (check DATABASE_URL)
- Port conflicts (9090, 9091, or 3000 already in use)

### Port conflicts

If you see "address already in use" errors:

**Find conflicting process:**
```bash
# Linux
sudo lsof -i :9090
sudo ss -tulpn | grep 9090

# macOS
lsof -i :9090
```

**Change ports in docker-compose.yml:**
```yaml
indexer:
  ports:
    - "9093:9090"  # Map host port 9093 to container port 9090
```

### Database connection errors

**Test connectivity from container:**
```bash
# Get a shell in the container
docker-compose exec indexer /bin/bash

# Test database connection
apt-get update && apt-get install -y postgresql-client
psql "your-database-url" -c "SELECT 1;"
```

**Common fixes:**
- Verify DATABASE_URL is correct in `.env`
- Check firewall rules allow outbound connections
- Ensure database accepts connections from your IP
- For NeonDB, verify SSL mode is set to `require`

### Prometheus not scraping metrics

**Check Prometheus targets:**
Visit http://localhost:9091/targets and look for the `mev-burn-indexer` job.

**If target shows as DOWN:**
1. Verify indexer container is healthy: `docker-compose ps`
2. Check metrics endpoint is accessible: `curl http://localhost:9090/metrics`
3. Ensure network connectivity between containers: `docker network inspect mev-burn-indexer_monitoring`

### Grafana shows "No data"

**Verify data source configuration:**
1. Log into Grafana at http://localhost:3000
2. Go to Configuration → Data Sources
3. Test the Prometheus data source
4. Check the PostgreSQL data source credentials

**Refresh dashboards:**
Dashboards may need 1 to 2 minutes to populate after initial startup. Click the refresh button or adjust the time range.

## Scaling and performance

### Resource limits

By default, containers have no resource limits. For production deployments, add limits to `docker-compose.yml`:

```yaml
indexer:
  deploy:
    resources:
      limits:
        cpus: '1'
        memory: 1G
      reservations:
        cpus: '0.5'
        memory: 512M
```

### Log rotation

The compose file includes log rotation configuration:
```yaml
logging:
  driver: "json-file"
  options:
    max-size: "10m"
    max-file: "3"
```

This keeps logs under control (max 30MB per service).

### Prometheus data retention

Prometheus retains 30 days of data by default. To change:

Edit `docker-compose.yml`:
```yaml
prometheus:
  command:
    - '--storage.tsdb.retention.time=90d'  # Keep 90 days
```

Monitor disk usage:
```bash
docker system df
du -sh $(docker volume inspect mev-burn-indexer_prometheus-data -f '{{.Mountpoint}}')
```

## Production deployment best practices

**Use secrets management**
Don't commit `.env` files. Use Docker secrets or external secrets managers:
```bash
# Example with Docker secrets
echo "your-db-password" | docker secret create db_password -
```

**Enable HTTPS**
Place a reverse proxy (nginx, Caddy) in front of Grafana and Prometheus:
```yaml
nginx:
  image: nginx:alpine
  volumes:
    - ./nginx.conf:/etc/nginx/nginx.conf
    - ./certs:/etc/nginx/certs
  ports:
    - "443:443"
```

**Set up automated backups**
```bash
# Backup Prometheus data
docker run --rm -v mev-burn-indexer_prometheus-data:/data \
  -v $(pwd):/backup alpine tar czf /backup/prometheus-backup.tar.gz /data

# Backup Grafana dashboards
docker run --rm -v mev-burn-indexer_grafana-data:/data \
  -v $(pwd):/backup alpine tar czf /backup/grafana-backup.tar.gz /data
```

**Monitor container health**
Use a monitoring tool like Portainer, cAdvisor, or cloud provider monitoring:
```bash
# Install Portainer for web-based container management
docker volume create portainer_data
docker run -d -p 9000:9000 --name portainer --restart always \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v portainer_data:/data portainer/portainer-ce
```

**Set up alerting**
Configure Prometheus alerts and Grafana notifications for critical failures:
- Stream disconnected for more than 5 minutes
- Transaction processing errors exceed 10%
- Database connection lost
- Container restart loops

## Cloud deployment

### AWS ECS

```bash
# Build for ARM64 if using Graviton instances
docker buildx build --platform linux/arm64 -t your-registry/mev-burn-indexer:latest .

# Push to ECR
aws ecr get-login-password --region us-east-1 | docker login --username AWS --password-stdin your-account.dkr.ecr.us-east-1.amazonaws.com
docker push your-registry/mev-burn-indexer:latest

# Deploy to ECS (use ECS task definition)
```

### Google Cloud Run

```bash
# Build and push to GCR
gcloud builds submit --tag gcr.io/your-project/mev-burn-indexer

# Deploy to Cloud Run
gcloud run deploy mev-burn-indexer \
  --image gcr.io/your-project/mev-burn-indexer \
  --platform managed \
  --region us-central1 \
  --set-env-vars="$(cat .env | tr '\n' ',')"
```

### DigitalOcean App Platform

Create `app.yaml`:
```yaml
name: mev-burn-indexer
services:
  - name: indexer
    dockerfile_path: Dockerfile
    github:
      repo: your-username/mev-burn-indexer
      branch: main
    envs:
      - key: DATABASE_URL
        scope: RUN_TIME
        value: ${DATABASE_URL}
```

Deploy:
```bash
doctl apps create --spec app.yaml
```

## Additional resources

- [Docker Compose documentation](https://docs.docker.com/compose/)
- [Dockerfile best practices](https://docs.docker.com/develop/develop-images/dockerfile_best-practices/)
- [Container security guidelines](https://cheatsheetseries.owasp.org/cheatsheets/Docker_Security_Cheat_Sheet.html)
