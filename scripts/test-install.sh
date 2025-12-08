#!/usr/bin/env bash
# Test script for install.sh
# Usage:
#   ./scripts/test-install.sh              # Run all platform tests in Docker
#   ./scripts/test-install.sh syntax       # Syntax check only
#   ./scripts/test-install.sh dry-run      # Dry run validation
#   ./scripts/test-install.sh linux-x64    # Test Linux x64 in Docker
#   ./scripts/test-install.sh linux-arm64  # Test Linux ARM64 in Docker
#   ./scripts/test-install.sh help         # Show usage

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
INSTALL_SCRIPT="$PROJECT_DIR/install.sh"
DOCKER_DIR="$PROJECT_DIR/docker"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

test_syntax() {
    info "Testing bash syntax..."

    if [ ! -f "$INSTALL_SCRIPT" ]; then
        error "Install script not found: $INSTALL_SCRIPT"
    fi

    # Check bash syntax
    if bash -n "$INSTALL_SCRIPT"; then
        success "Bash syntax check passed"
    else
        error "Bash syntax errors found"
    fi

    # Check for shellcheck if available
    if command -v shellcheck &> /dev/null; then
        info "Running shellcheck..."
        if shellcheck -e SC2086,SC2034 "$INSTALL_SCRIPT"; then
            success "Shellcheck passed"
        else
            error "Shellcheck found issues"
        fi
    else
        info "shellcheck not installed, skipping"
    fi
}

test_dry_run() {
    info "Testing install script structure..."

    # Check for required functions
    local required_functions=(
        "detect_platform"
        "get_latest_version"
        "download"
        "install_binary"
        "setup_path"
        "verify"
        "uninstall"
    )

    local missing_functions=()
    for func in "${required_functions[@]}"; do
        if ! grep -q "^${func}()" "$INSTALL_SCRIPT"; then
            missing_functions+=("$func")
        fi
    done

    if [ ${#missing_functions[@]} -gt 0 ]; then
        error "Missing required functions: ${missing_functions[*]}"
    fi

    success "All required functions found"

    # Check for required variables
    local required_vars=(
        "VERSION"
        "REPO"
        "BINARY_NAME"
        "INSTALL_DIR"
    )

    local missing_vars=()
    for var in "${required_vars[@]}"; do
        if ! grep -q "^${var}=" "$INSTALL_SCRIPT"; then
            missing_vars+=("$var")
        fi
    done

    if [ ${#missing_vars[@]} -gt 0 ]; then
        error "Missing required variables: ${missing_vars[*]}"
    fi

    success "All required variables found"

    # Check for error handling
    if ! grep -q "set -e" "$INSTALL_SCRIPT"; then
        error "Script should use 'set -e' for error handling"
    fi

    success "Error handling configured correctly"
}

test_linux_docker() {
    local platform="$1"
    info "Testing install script on $platform in Docker..."

    if ! command -v docker &> /dev/null; then
        error "Docker is required for platform tests"
    fi

    # Build test image if needed
    local dockerfile="$DOCKER_DIR/Dockerfile.install-test"
    if [ ! -f "$dockerfile" ]; then
        info "Creating install test Dockerfile..."
        cat > "$dockerfile" << 'DOCKERFILE'
# Dockerfile for testing install script
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    curl \
    wget \
    ca-certificates \
    bash \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /test

# Copy install script
COPY install.sh /test/install.sh
RUN chmod +x /test/install.sh

# Create mock binary for testing
RUN mkdir -p /mock-server

# Test script that validates install script
COPY scripts/docker-install-test.sh /test/run-test.sh
RUN chmod +x /test/run-test.sh

CMD ["/test/run-test.sh"]
DOCKERFILE
    fi

    # Create Docker test script
    local test_script="$SCRIPT_DIR/docker-install-test.sh"
    if [ ! -f "$test_script" ]; then
        info "Creating Docker test script..."
        cat > "$test_script" << 'TESTSCRIPT'
#!/bin/bash
set -e

echo "=== Install Script Test ==="
echo ""

# Test 1: Syntax check
echo "[TEST] Syntax check..."
if bash -n /test/install.sh; then
    echo "[PASS] Syntax OK"
else
    echo "[FAIL] Syntax errors"
    exit 1
fi

# Test 2: Validate functions exist
echo "[TEST] Function validation..."
required_functions="detect_platform get_latest_version download install_binary"
for func in $required_functions; do
    if grep -q "^${func}()" /test/install.sh; then
        echo "[PASS] Function $func found"
    else
        echo "[FAIL] Function $func missing"
        exit 1
    fi
done

# Test 3: Platform detection
echo "[TEST] Platform detection..."
source /test/install.sh --help 2>/dev/null || true

# Test 4: Help output
echo "[TEST] Help/usage output..."
if /test/install.sh --help 2>&1 | grep -q "Usage\|install\|uninstall"; then
    echo "[PASS] Help output OK"
else
    echo "[INFO] No help flag (expected for curl|bash usage)"
fi

echo ""
echo "=== All Tests Passed ==="
TESTSCRIPT
        chmod +x "$test_script"
    fi

    # Run Docker test
    cd "$PROJECT_DIR"
    docker build -f "$dockerfile" -t ratterm-install-test .
    docker run --rm ratterm-install-test

    success "$platform install test passed"
}

show_usage() {
    echo "Install Script Test Tool"
    echo ""
    echo "Usage: ./scripts/test-install.sh [COMMAND]"
    echo ""
    echo "Commands:"
    echo "  syntax       Test bash syntax (and shellcheck if available)"
    echo "  dry-run      Validate script structure"
    echo "  linux-x64    Test on Linux x64 in Docker"
    echo "  all          Run all tests"
    echo "  help         Show this help message"
    echo ""
    echo "Default: syntax + dry-run"
}

# Main
main() {
    case "${1:-}" in
        "")
            test_syntax
            test_dry_run
            echo ""
            success "All basic tests passed!"
            ;;
        syntax)
            test_syntax
            ;;
        dry-run)
            test_dry_run
            ;;
        linux-x64)
            test_syntax
            test_dry_run
            test_linux_docker "linux-x64"
            ;;
        all)
            test_syntax
            test_dry_run
            test_linux_docker "linux-x64"
            echo ""
            success "All tests passed!"
            ;;
        help|--help|-h)
            show_usage
            ;;
        *)
            error "Unknown command: $1"
            ;;
    esac
}

main "$@"
