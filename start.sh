#!/usr/bin/env bash
#
# MEV Burn Indexer - Application Startup Script
#
# This script performs a complete startup sequence:
# 1. Validates prerequisites (Rust toolchain, environment configuration)
# 2. Builds the application in release mode
# 3. Runs the application with structured logging to console
#
# Usage:
#   ./start.sh           # Start with info-level logging
#   LOG_LEVEL=debug ./start.sh  # Start with debug-level logging
#
# Exit codes:
#   0 - Success
#   1 - Missing prerequisites
#   2 - Build failure
#   3 - Runtime error

set -euo pipefail

# Color codes for output
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly NC='\033[0m' # No Color

# Log functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $*"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $*"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*" >&2
}

log_header() {
    echo
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  $*${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo
}

# Check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Validate prerequisites
validate_prerequisites() {
    log_header "Validating Prerequisites"
    
    local has_errors=0
    
    # Check for Rust toolchain
    if ! command_exists cargo; then
        log_error "Rust toolchain not found. Install from https://rustup.rs/"
        has_errors=1
    else
        local rust_version
        rust_version=$(rustc --version | cut -d' ' -f2)
        log_success "Rust toolchain found (version $rust_version)"
    fi
    
    # Check for .env file
    if [[ ! -f .env ]]; then
        log_error ".env file not found. Please create one with required configuration."
        log_info "Required variables: GRPC_ENDPOINT, GRPC_TOKEN, TARGET_ACCOUNT, DATABASE_URL"
        has_errors=1
    else
        log_success ".env file found"
        
        # Validate required environment variables by checking if they exist in the .env file
        local required_vars=("GRPC_ENDPOINT" "TARGET_ACCOUNT" "DATABASE_URL")
        for var in "${required_vars[@]}"; do
            if grep -q "^${var}=" .env 2>/dev/null; then
                log_success "Environment variable $var is set"
            else
                log_error "Required environment variable $var is not set in .env"
                has_errors=1
            fi
        done
    fi
    
    # Check for src directory
    if [[ ! -d src ]]; then
        log_error "src directory not found. Are you in the project root?"
        has_errors=1
    else
        log_success "Source directory found"
    fi
    
    # Check for Cargo.toml
    if [[ ! -f Cargo.toml ]]; then
        log_error "Cargo.toml not found. Are you in the project root?"
        has_errors=1
    else
        log_success "Cargo.toml found"
    fi
    
    if [[ $has_errors -ne 0 ]]; then
        log_error "Prerequisites validation failed"
        return 1
    fi
    
    log_success "All prerequisites validated"
    return 0
}

# Build the application
build_application() {
    log_header "Building Application"
    
    log_info "Running cargo build --release..."
    log_info "This may take a few minutes on first build..."
    
    if cargo build --release 2>&1; then
        log_success "Build completed successfully"
        return 0
    else
        log_error "Build failed"
        return 1
    fi
}

# Run the application
run_application() {
    log_header "Starting Application"
    
    # Load environment variables
    if [[ -f .env ]]; then
        # Export all variables from .env
        set -a
        source .env
        set +a
    fi
    
    # Set default log level if not specified
    export LOG_LEVEL="${LOG_LEVEL:-info}"
    
    log_info "Target Account: ${TARGET_ACCOUNT:-<not set>}"
    log_info "gRPC Endpoint: ${GRPC_ENDPOINT:-<not set>}"
    # Show DB URL without query params, using a safer approach
    if [[ -n "${DATABASE_URL:-}" ]]; then
        log_info "Database: ${DATABASE_URL%%\?*}"
    else
        log_info "Database: <not set>"
    fi
    log_info "Log Level: $LOG_LEVEL"
    echo
    
    log_success "Application starting..."
    log_info "Press Ctrl+C to stop"
    echo
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  Application Logs${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo
    
    # Run the application
    # The application's tracing will handle all logging to console
    ./target/release/mev-burn-indexer
}

# Cleanup on exit
cleanup() {
    local exit_code=$?
    echo
    if [[ $exit_code -eq 0 ]]; then
        log_info "Application stopped gracefully"
    else
        log_warning "Application stopped with exit code $exit_code"
    fi
}

trap cleanup EXIT

# Main execution
main() {
    log_header "MEV Burn Indexer"
    
    # Validate prerequisites
    if ! validate_prerequisites; then
        exit 1
    fi
    
    # Build application
    if ! build_application; then
        exit 2
    fi
    
    # Run application
    if ! run_application; then
        exit 3
    fi
}

# Execute main function
main "$@"
