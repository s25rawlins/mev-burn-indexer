#!/usr/bin/env bash

# Performs a complete startup sequence:
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

readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly NC='\033[0m' # No Color

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

command_exists() {
    command -v "$1" >/dev/null 2>&1
}

validate_env_file() {
    if [[ ! -f .env ]]; then
        log_error ".env file not found. Please create one with required configuration."
        log_info "Tip: Copy .env.example to .env and fill in your values:"
        log_info "  cp .env.example .env"
        return 1
    fi
    
    if [[ ! -r .env ]]; then
        log_error ".env file exists but is not readable. Check file permissions."
        log_info "Fix with: chmod 644 .env"
        return 1
    fi
    
    log_success ".env file found"
    return 0
}

validate_env_syntax() {
    local line_num=0
    local has_errors=0
    
    while IFS= read -r line || [[ -n "$line" ]]; do
        ((line_num++))
        
        [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue
        
        if ! [[ "$line" =~ ^[A-Za-z_][A-Za-z0-9_]*= ]]; then
            log_error ".env file has invalid syntax at line $line_num: $line"
            log_info "Expected format: KEY=value"
            has_errors=1
        fi
    done < .env
    
    if [[ $has_errors -ne 0 ]]; then
        return 1
    fi
    
    return 0
}

validate_env_variables() {
    local required_vars=("GRPC_ENDPOINT" "TARGET_ACCOUNT" "DATABASE_URL")
    local has_errors=0
    
    for var in "${required_vars[@]}"; do
        if ! grep -q "^${var}=" .env 2>/dev/null; then
            log_error "Required environment variable $var is not defined in .env"
            log_info "Add to .env: $var=<your-value>"
            has_errors=1
        elif ! grep "^${var}=" .env | grep -q "=."; then
            log_error "Environment variable $var is defined but has no value"
            log_info "Set a value in .env: $var=<your-value>"
            has_errors=1
        else
            log_success "Environment variable $var is set"
        fi
    done
    
    if [[ $has_errors -ne 0 ]]; then
        return 1
    fi
    
    return 0
}

validate_prerequisites() {
    log_header "Validating Prerequisites"
    
    local has_errors=0
    
    if ! command_exists cargo; then
        log_error "Rust toolchain not found."
        log_info "Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        log_info "Or visit: https://rustup.rs/"
        has_errors=1
    else
        local rust_version
        rust_version=$(rustc --version | cut -d' ' -f2)
        log_success "Rust toolchain found (version $rust_version)"
    fi

    if ! validate_env_file; then
        has_errors=1
    elif ! validate_env_syntax; then
        has_errors=1
    elif ! validate_env_variables; then
        has_errors=1
    fi

    if [[ ! -d src ]]; then
        log_error "src directory not found. Are you in the project root?"
        log_info "Navigate to project root: cd /path/to/mev-burn-indexer"
        has_errors=1
    else
        log_success "Source directory found"
    fi
    
    if [[ ! -f Cargo.toml ]]; then
        log_error "Cargo.toml not found. Are you in the project root?"
        log_info "Navigate to project root: cd /path/to/mev-burn-indexer"
        has_errors=1
    else
        log_success "Cargo.toml found"
    fi
    
    if [[ $has_errors -ne 0 ]]; then
        log_error "Prerequisites validation failed. Please address the issues above."
        return 1
    fi
    
    log_success "All prerequisites validated"
    return 0
}

build_application() {
    log_header "Building Application"
    
    log_info "Running cargo build --release..."
    log_info "This may take a few minutes on first build..."
    
    local build_output
    local build_status
    
    if build_output=$(cargo build --release 2>&1); then
        build_status=0
    else
        build_status=$?
    fi
    
    if [[ -n "$build_output" ]]; then
        echo "$build_output"
    fi
    
    if [[ $build_status -ne 0 ]]; then
        log_error "Build failed with exit code $build_status"
        log_info "Check the error messages above for details."
        log_info "Common fixes:"
        log_info "  - Run 'cargo clean' to clear build cache"
        log_info "  - Check your Rust toolchain is up to date: rustup update"
        return 1
    fi

    # Verify the binary was created (catches workspace configuration issues)
    if [[ ! -f target/release/mev-burn-indexer ]]; then
        log_error "Build reported success but binary not found at target/release/mev-burn-indexer"
        log_error "This may indicate a workspace configuration issue in Cargo.toml"
        return 1
    fi
    
    if [[ ! -x target/release/mev-burn-indexer ]]; then
        log_error "Binary exists but is not executable. Check file permissions."
        log_info "Fix with: chmod +x target/release/mev-burn-indexer"
        return 1
    fi
    
    log_success "Build completed successfully"
    return 0
}

load_environment() {
    if [[ ! -f .env ]]; then
        log_warning ".env file not found at runtime. Using environment defaults."
        return 1
    fi
    
    # Test sourcing in a subshell first to catch any runtime errors
    # This prevents the main script from crashing if .env has issues
    if ! (set -a; source .env; set +a) 2>/dev/null; then
        log_error "Failed to load .env file. It may contain invalid shell syntax."
        log_info "Check for special characters that need escaping in values"
        return 1
    fi
    
    set -a
    source .env
    set +a
    
    return 0
}

run_application() {
    log_header "Starting Application"
    
    # Verify binary still exists (guards against cleanup between build and run)
    if [[ ! -f target/release/mev-burn-indexer ]]; then
        log_error "Application binary not found at target/release/mev-burn-indexer"
        log_error "Build artifacts may have been cleaned. Try running the script again."
        return 1
    fi
    
    if [[ ! -x target/release/mev-burn-indexer ]]; then
        log_error "Application binary is not executable"
        log_info "Fix with: chmod +x target/release/mev-burn-indexer"
        return 1
    fi
    
    load_environment
    
    export LOG_LEVEL="${LOG_LEVEL:-info}"
    
    # Validate critical environment variables are now set after sourcing
    if [[ -z "${TARGET_ACCOUNT:-}" ]]; then
        log_error "TARGET_ACCOUNT is not set. Cannot start application."
        log_info "Ensure TARGET_ACCOUNT is defined in .env file"
        return 1
    fi
    
    if [[ -z "${GRPC_ENDPOINT:-}" ]]; then
        log_error "GRPC_ENDPOINT is not set. Cannot start application."
        log_info "Ensure GRPC_ENDPOINT is defined in .env file"
        return 1
    fi
    
    if [[ -z "${DATABASE_URL:-}" ]]; then
        log_error "DATABASE_URL is not set. Cannot start application."
        log_info "Ensure DATABASE_URL is defined in .env file"
        return 1
    fi
    
    log_info "Target Account: ${TARGET_ACCOUNT}"
    log_info "gRPC Endpoint: ${GRPC_ENDPOINT}"
    
    # Show DB URL without query params to avoid exposing credentials
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
    
    # The application's tracing will handle all logging to console
    ./target/release/mev-burn-indexer
}

# Cleanup function called on script exit
# Handles EXIT, ERR, and INT signals
cleanup() {
    local exit_code=$?
    echo
    if [[ $exit_code -eq 0 ]]; then
        log_info "Application stopped gracefully"
    elif [[ $exit_code -eq 130 ]]; then
        log_info "Application interrupted by user (Ctrl+C)"
    else
        log_warning "Application stopped with exit code $exit_code"
    fi
}

trap cleanup EXIT ERR INT

# Orchestrates the full startup sequence
main() {
    log_header "MEV Burn Indexer"
    
    if ! validate_prerequisites; then
        exit 1
    fi
    
    if ! build_application; then
        exit 2
    fi
    
    if ! run_application; then
        exit 3
    fi
}

main "$@"
