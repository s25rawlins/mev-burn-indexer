#!/usr/bin/env bash
set -euo pipefail

# Export sample data from MEV Burn Indexer database
# This script creates CSV exports of transaction and balance change data

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly OUTPUT_DIR="${SCRIPT_DIR}/sample_data"

mkdir -p "${OUTPUT_DIR}"

if [[ -f "${SCRIPT_DIR}/.env" ]]; then
    set -a
    source "${SCRIPT_DIR}/.env"
    set +a
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
    echo "ERROR: DATABASE_URL environment variable is not set" >&2
    echo "Please set it in your .env file or export it" >&2
    echo "" >&2
    echo "Example format: postgresql://user:password@host:port/mev-burn-indexer" >&2
    exit 1
fi

echo "Exporting sample data from database..."

echo "Exporting transactions table (MEV bot program interactions)..."
psql "${DATABASE_URL}" -c \
    "COPY (SELECT * FROM transactions ORDER BY block_time DESC LIMIT 1000) TO STDOUT WITH CSV HEADER" \
    > "${OUTPUT_DIR}/sample_transactions.csv"

if [[ $? -eq 0 ]]; then
    echo "✓ Transactions exported to ${OUTPUT_DIR}/sample_transactions.csv"
else
    echo "✗ Failed to export transactions" >&2
    exit 1
fi

echo "Exporting account_balance_changes table (for MEV bot program interactions)..."
psql "${DATABASE_URL}" -c \
    "COPY (SELECT * FROM account_balance_changes ORDER BY id DESC LIMIT 1000) TO STDOUT WITH CSV HEADER" \
    > "${OUTPUT_DIR}/sample_balance_changes.csv"

if [[ $? -eq 0 ]]; then
    echo "✓ Balance changes exported to ${OUTPUT_DIR}/sample_balance_changes.csv"
else
    echo "✗ Failed to export balance changes" >&2
    exit 1
fi

echo "Creating summary statistics (MEV bot program interactions)..."
psql "${DATABASE_URL}" -c \
    "SELECT 
        COUNT(*) as total_transactions,
        COUNT(*) FILTER (WHERE success = true) as successful,
        COUNT(*) FILTER (WHERE success = false) as failed,
        SUM(fee) as total_burn_lamports,
        MIN(block_time) as earliest_transaction,
        MAX(block_time) as latest_transaction
    FROM transactions;" \
    > "${OUTPUT_DIR}/summary_statistics.txt"

echo "✓ Summary statistics exported to ${OUTPUT_DIR}/summary_statistics.txt"

echo ""
echo "Export complete! Files are in: ${OUTPUT_DIR}/"
echo "You can now add these files to your repository for submission."
