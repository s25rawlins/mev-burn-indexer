#!/bin/bash

# Grafana User Provisioning Script
#
# Creates a Grafana user with specified credentials after the Grafana 
# container has started. Uses the Grafana API to add the user and assign 
# appropriate permissions.
#
# Usage:
#   ./create_grafana_user.sh [EMAIL] [PASSWORD] [NAME] [LOGIN]
#
# Arguments can also be provided via environment variables:
#   GRAFANA_USER_EMAIL
#   GRAFANA_USER_PASSWORD
#   GRAFANA_USER_NAME
#   GRAFANA_USER_LOGIN
#
# If no arguments or environment variables are provided, the script exits.

set -e

GRAFANA_URL="${GRAFANA_URL:-http://localhost:3000}"
ADMIN_USER="${ADMIN_USER:-admin}"
ADMIN_PASSWORD="${ADMIN_PASSWORD:-admin}"

USER_EMAIL="${1:-${GRAFANA_USER_EMAIL}}"
USER_PASSWORD="${2:-${GRAFANA_USER_PASSWORD}}"
USER_NAME="${3:-${GRAFANA_USER_NAME}}"
USER_LOGIN="${4:-${GRAFANA_USER_LOGIN:-${USER_EMAIL}}}"

if [ -z "$USER_EMAIL" ] || [ -z "$USER_PASSWORD" ]; then
    echo "Error: User email and password are required"
    echo ""
    echo "Usage:"
    echo "  $0 EMAIL PASSWORD [NAME] [LOGIN]"
    echo ""
    echo "Or set environment variables:"
    echo "  GRAFANA_USER_EMAIL"
    echo "  GRAFANA_USER_PASSWORD"
    echo "  GRAFANA_USER_NAME (optional)"
    echo "  GRAFANA_USER_LOGIN (optional, defaults to email)"
    exit 1
fi

if [ -z "$USER_NAME" ]; then
    USER_NAME="$USER_EMAIL"
fi

echo "Provisioning Grafana user: $USER_EMAIL"

echo "Waiting for Grafana to be ready..."
max_attempts=30
attempt=0

while [ $attempt -lt $max_attempts ]; do
    if curl -s -o /dev/null -w "%{http_code}" "${GRAFANA_URL}/api/health" | grep -q "200"; then
        echo "Grafana is ready"
        break
    fi
    attempt=$((attempt + 1))
    echo "Attempt $attempt/$max_attempts: Grafana not ready yet, waiting..."
    sleep 2
done

if [ $attempt -eq $max_attempts ]; then
    echo "Error: Grafana did not become ready in time"
    exit 1
fi

echo "Checking if user exists..."
USER_EXISTS=$(curl -s -u "${ADMIN_USER}:${ADMIN_PASSWORD}" \
    "${GRAFANA_URL}/api/users/lookup?loginOrEmail=${USER_EMAIL}" \
    | grep -o '"id":[0-9]*' || echo "")

if [ -n "$USER_EXISTS" ]; then
    echo "User ${USER_EMAIL} already exists, skipping creation"
    
    USER_ID=$(echo "$USER_EXISTS" | grep -o '[0-9]*')
    echo "User ID: $USER_ID"
    
    echo "Updating user password..."
    curl -s -X PUT \
        -u "${ADMIN_USER}:${ADMIN_PASSWORD}" \
        -H "Content-Type: application/json" \
        -d "{\"password\": \"${USER_PASSWORD}\"}" \
        "${GRAFANA_URL}/api/admin/users/${USER_ID}/password"
    
    echo "Password updated successfully"
else
    echo "Creating user ${USER_EMAIL}..."
    RESPONSE=$(curl -s -X POST \
        -u "${ADMIN_USER}:${ADMIN_PASSWORD}" \
        -H "Content-Type: application/json" \
        -d "{
            \"name\": \"${USER_NAME}\",
            \"email\": \"${USER_EMAIL}\",
            \"login\": \"${USER_LOGIN}\",
            \"password\": \"${USER_PASSWORD}\",
            \"OrgId\": 1
        }" \
        "${GRAFANA_URL}/api/admin/users")
    
    if echo "$RESPONSE" | grep -q '"id"'; then
        echo "User created successfully"
        
        USER_ID=$(echo "$RESPONSE" | grep -o '"id":[0-9]*' | grep -o '[0-9]*')
        
        echo "Granting admin permissions to user..."
        curl -s -X PATCH \
            -u "${ADMIN_USER}:${ADMIN_PASSWORD}" \
            -H "Content-Type: application/json" \
            -d "{\"isGrafanaAdmin\": true}" \
            "${GRAFANA_URL}/api/admin/users/${USER_ID}/permissions"
        
        echo "Admin permissions granted successfully"
    else
        echo "Error creating user: $RESPONSE"
        exit 1
    fi
fi

echo "User provisioning complete"
echo "Login credentials:"
echo "  Email: ${USER_EMAIL}"
echo "  Login: ${USER_LOGIN}"
