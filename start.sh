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
    elif [[ ! -r .env ]]; then
        # Verify file is readable to prevent source failures
        log_error ".env file exists but is not readable. Check file permissions."
        has_errors=1
    else
        log_success ".env file found"
        
        # Validate .env syntax before attempting to source it
        if ! bash -n .env 2>/dev/null; then
            log_error ".env file contains syntax errors. Please fix and try again."
            has_errors=1
        else
            # Validate required environment variables exist and have non-empty values
            # We load the .env temporarily to check values without polluting current environment
            local required_vars=("GRPC_ENDPOINT" "TARGET_ACCOUNT" "DATABASE_URL")
            for var in "${required_vars[@]}"; do
                # Check if variable line exists in .env
                if ! grep -q "^${var}=" .env 2>/dev/null; then
                    log_error "Required environment variable $var is not defined in .env"
                    has_errors=1
                elif ! grep "^${var}=" .env | grep -q "=."; then
                    # Verify the value after = is not empty
                    log_error "Environment variable $var is defined but has no value"
                    has_errors=1
                else
                    log_success "Environment variable $var is set"
                fi
            done
        fi
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
    
    # Capture build output and exit status
    local build_output
    local build_status
    
    if build_output=$(cargo build --release 2>&1); then
        build_status=0
    else
        build_status=$?
    fi
    
    # Display any build output for transparency
    if [[ -n "$build_output" ]]; then
        echo "$build_output"
    fi
    
    if [[ $build_status -ne 0 ]]; then
        log_error "Build failed with exit code $build_status"
        return 1
    fi
    
    # Verify the binary was actually created
    # This catches edge cases where cargo succeeds but doesn't produce the expected artifact
    if [[ ! -f target/release/mev-burn-indexer ]]; then
        log_error "Build reported success but binary not found at target/release/mev-burn-indexer"
        log_error "This may indicate a workspace configuration issue"
        return 1
    fi
    
    # Verify the binary is executable
    if [[ ! -x target/release/mev-burn-indexer ]]; then
        log_error "Binary exists but is not executable. Check file permissions."
        return 1
    fi
    
    log_success "Build completed successfully"
    return 0
}

# Run the application
run_application() {
    log_header "Starting Application"
    
    # Final verification that binary exists before attempting to run
    # This guards against scenarios where build artifacts were cleaned between build and run
    if [[ ! -f target/release/mev-burn-indexer ]]; then
        log_error "Application binary not found at target/release/mev-burn-indexer"
        log_error "Build artifacts may have been cleaned. Try running the script again."
        return 1
    fi
    
    if [[ ! -x target/release/mev-burn-indexer ]]; then
        log_error "Application binary is not executable"
        return 1
    fi
    
    # Load environment variables with error handling
    if [[ -f .env ]]; then
        # Wrap sourcing in a subshell test first to catch any runtime errors
        # This prevents the main script from crashing if .env has issues
        if (set -a; source .env; set +a) 2>/dev/null; then
            # Safe to source in main shell
            set -a
            source .env
            set +a
        else
            log_error "Failed to load .env file. It may contain invalid shell syntax."
            return 1
        fi
    else
        log_warning ".env file not found at runtime. Using environment defaults."
    fi
    
    # Set default log level if not specified
    export LOG_LEVEL="${LOG_LEVEL:-info}"
    
    # Validate critical environment variables are now set after sourcing
    # These are required for the application to function
    if [[ -z "${TARGET_ACCOUNT:-}" ]]; then
        log_error "TARGET_ACCOUNT is not set. Cannot start application."
        return 1
    fi
    
    if [[ -z "${GRPC_ENDPOINT:-}" ]]; then
        log_error "GRPC_ENDPOINT is not set. Cannot start application."
        return 1
    fi
    
    if [[ -z "${DATABASE_URL:-}" ]]; then
        log_error "DATABASE_URL is not set. Cannot start application."
        return 1
    fi
    
    log_info "Target Account: ${TARGET_ACCOUNT}"
    log_info "gRPC Endpoint: ${GRPC_ENDPOINT}"
    # Show DB URL without query params (credentials)
    # Using parameter expansion to safely extract just the connection string base
    local db_display="${DATABASE_URL%%\?*}"
    log_info "Database: ${db_display}"
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
