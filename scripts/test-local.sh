#!/usr/bin/env bash
# Local CI testing script using Docker
# Mirrors the GitHub Actions CI environment
#
# Usage:
#   ./scripts/test-local.sh          # Run all CI checks
#   ./scripts/test-local.sh fmt      # Format check only
#   ./scripts/test-local.sh clippy   # Clippy lints only
#   ./scripts/test-local.sh test     # Tests only
#   ./scripts/test-local.sh docs     # Documentation only
#   ./scripts/test-local.sh audit    # Security audit only
#   ./scripts/test-local.sh msrv     # MSRV check only
#   ./scripts/test-local.sh clean    # Clean up Docker volumes

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
DOCKER_DIR="$PROJECT_DIR/docker"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if Docker is available
check_docker() {
    if ! command -v docker &> /dev/null; then
        error "Docker is not installed or not in PATH"
        echo "Please install Docker: https://docs.docker.com/get-docker/"
        exit 1
    fi

    if ! docker info &> /dev/null; then
        error "Docker daemon is not running"
        echo "Please start Docker and try again"
        exit 1
    fi

    info "Docker is available"
}

# Run a specific test service
run_service() {
    local service=$1
    info "Running: $service"

    cd "$PROJECT_DIR"
    docker compose -f docker/docker-compose.yml up --build --abort-on-container-exit "$service"
    local exit_code=$?

    if [ $exit_code -eq 0 ]; then
        success "$service passed"
    else
        error "$service failed with exit code $exit_code"
        exit $exit_code
    fi
}

# Run all CI checks
run_all() {
    info "Running all CI checks..."
    echo ""

    local checks=("fmt" "clippy" "test" "docs" "audit" "msrv")
    local failed=()

    for check in "${checks[@]}"; do
        echo ""
        echo "=========================================="
        info "Running: $check"
        echo "=========================================="

        cd "$PROJECT_DIR"
        if docker compose -f docker/docker-compose.yml up --build --abort-on-container-exit "$check"; then
            success "$check passed"
        else
            error "$check failed"
            failed+=("$check")
        fi
    done

    echo ""
    echo "=========================================="
    echo "                SUMMARY                   "
    echo "=========================================="

    if [ ${#failed[@]} -eq 0 ]; then
        success "All checks passed!"
    else
        error "Failed checks: ${failed[*]}"
        exit 1
    fi
}

# Clean up Docker resources
clean() {
    info "Cleaning up Docker resources..."
    cd "$PROJECT_DIR"
    docker compose -f docker/docker-compose.yml down -v --rmi local
    success "Cleanup complete"
}

# Print usage
usage() {
    echo "Usage: $0 [COMMAND]"
    echo ""
    echo "Commands:"
    echo "  (none)       Run all CI checks"
    echo "  fmt          Format check only"
    echo "  clippy       Clippy lints only"
    echo "  test         Tests only"
    echo "  docs         Documentation only"
    echo "  audit        Security audit only"
    echo "  msrv         MSRV check only"
    echo "  ci-all       Run all checks in one container"
    echo "  install-test Test install script in Docker"
    echo "  lua-test     Run Lua extension tests"
    echo "  lua-test-arm Run Lua extension tests on ARM64 (QEMU)"
    echo "  test-arm     Run tests on ARM64 Linux (QEMU)"
    echo "  ci-all-arm   Run all checks on ARM64 Linux (QEMU)"
    echo "  clean        Clean up Docker volumes and images"
    echo "  help         Show this help message"
}

# Main
main() {
    check_docker

    case "${1:-}" in
        "")
            run_all
            ;;
        fmt|clippy|test|docs|audit|msrv|ci-all|install-test|lua-test|lua-test-arm|test-arm|ci-all-arm)
            run_service "$1"
            ;;
        clean)
            clean
            ;;
        help|--help|-h)
            usage
            ;;
        *)
            error "Unknown command: $1"
            usage
            exit 1
            ;;
    esac
}

main "$@"
