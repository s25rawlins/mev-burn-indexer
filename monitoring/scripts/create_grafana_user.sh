#!/bin/bash

# Grafana User Provisioning Script
#
# This script creates a Grafana user with specific credentials after the 
# Grafana container has started. It uses the Grafana API to add the user
# and assign appropriate permissions.

set -e

GRAFANA_URL="${GRAFANA_URL:-http://localhost:3000}"
ADMIN_USER="${ADMIN_USER:-admin}"
ADMIN_PASSWORD="${ADMIN_PASSWORD:-admin}"

USER_EMAIL="srawlins@gmail.com"
USER_LOGIN="srawlins@gmail.com"
USER_PASSWORD="2501"
USER_NAME="Sean Rawlins"

# Wait for Grafana to be ready
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

# Check if user already exists
echo "Checking if user exists..."
USER_EXISTS=$(curl -s -u "${ADMIN_USER}:${ADMIN_PASSWORD}" \
    "${GRAFANA_URL}/api/users/lookup?loginOrEmail=${USER_EMAIL}" \
    | grep -o '"id":[0-9]*' || echo "")

if [ -n "$USER_EXISTS" ]; then
    echo "User ${USER_EMAIL} already exists, skipping creation"
    
    # Extract user ID
    USER_ID=$(echo "$USER_EXISTS" | grep -o '[0-9]*')
    echo "User ID: $USER_ID"
    
    # Update password if needed
    echo "Updating user password..."
    curl -s -X PUT \
        -u "${ADMIN_USER}:${ADMIN_PASSWORD}" \
        -H "Content-Type: application/json" \
        -d "{\"password\": \"${USER_PASSWORD}\"}" \
        "${GRAFANA_URL}/api/admin/users/${USER_ID}/password"
    
    echo "Password updated successfully"
else
    # Create new user
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
        
        # Extract user ID from response
        USER_ID=$(echo "$RESPONSE" | grep -o '"id":[0-9]*' | grep -o '[0-9]*')
        
        # Grant admin role to user
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
echo "  Password: ${USER_PASSWORD}"
