#!/bin/bash
# Ratterm Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/OWNER/ratterm/main/install.sh | bash

set -e

VERSION="0.1.0"
REPO="hastur-dev/ratterm"
BINARY_NAME="rat"
INSTALL_DIR="${RATTERM_INSTALL_DIR:-$HOME/.local/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Detect OS and architecture
detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux*)  OS_NAME="linux" ;;
        Darwin*) OS_NAME="macos" ;;
        MINGW*|MSYS*|CYGWIN*) OS_NAME="windows" ;;
        *) error "Unsupported operating system: $OS" ;;
    esac

    case "$ARCH" in
        x86_64|amd64) ARCH_NAME="x86_64" ;;
        arm64|aarch64) ARCH_NAME="aarch64" ;;
        *) error "Unsupported architecture: $ARCH" ;;
    esac

    if [ "$OS_NAME" = "windows" ]; then
        ASSET_NAME="${BINARY_NAME}-${OS_NAME}-${ARCH_NAME}.exe"
    else
        ASSET_NAME="${BINARY_NAME}-${OS_NAME}-${ARCH_NAME}"
    fi

    info "Detected platform: $OS_NAME-$ARCH_NAME"
}

# Get latest version from GitHub
get_latest_version() {
    if command -v curl &> /dev/null; then
        LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')
    elif command -v wget &> /dev/null; then
        LATEST=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')
    else
        error "Neither curl nor wget found. Please install one of them."
    fi

    if [ -n "$LATEST" ]; then
        VERSION="$LATEST"
        info "Latest version: v$VERSION"
    else
        warn "Could not fetch latest version, using default: v$VERSION"
    fi
}

# Download binary
download() {
    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/v${VERSION}/${ASSET_NAME}"
    TEMP_FILE=$(mktemp)

    info "Downloading ${BINARY_NAME} v${VERSION}..."

    if command -v curl &> /dev/null; then
        curl -fsSL "$DOWNLOAD_URL" -o "$TEMP_FILE" || error "Download failed. Check if release exists."
    elif command -v wget &> /dev/null; then
        wget -q "$DOWNLOAD_URL" -O "$TEMP_FILE" || error "Download failed. Check if release exists."
    fi

    echo "$TEMP_FILE"
}

# Install binary
install_binary() {
    TEMP_FILE="$1"

    # Create install directory
    mkdir -p "$INSTALL_DIR"

    # Install binary
    if [ "$OS_NAME" = "windows" ]; then
        DEST="${INSTALL_DIR}/${BINARY_NAME}.exe"
    else
        DEST="${INSTALL_DIR}/${BINARY_NAME}"
    fi

    mv "$TEMP_FILE" "$DEST"
    chmod +x "$DEST"

    success "Installed ${BINARY_NAME} to ${DEST}"
}

# Add to PATH
setup_path() {
    # Check if already in PATH
    if echo "$PATH" | grep -q "$INSTALL_DIR"; then
        return
    fi

    SHELL_NAME=$(basename "$SHELL")
    case "$SHELL_NAME" in
        bash)
            RC_FILE="$HOME/.bashrc"
            ;;
        zsh)
            RC_FILE="$HOME/.zshrc"
            ;;
        fish)
            RC_FILE="$HOME/.config/fish/config.fish"
            ;;
        *)
            RC_FILE=""
            ;;
    esac

    if [ -n "$RC_FILE" ]; then
        if ! grep -q "$INSTALL_DIR" "$RC_FILE" 2>/dev/null; then
            echo "" >> "$RC_FILE"
            echo "# Ratterm" >> "$RC_FILE"
            if [ "$SHELL_NAME" = "fish" ]; then
                echo "set -gx PATH \$PATH $INSTALL_DIR" >> "$RC_FILE"
            else
                echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$RC_FILE"
            fi
            info "Added $INSTALL_DIR to PATH in $RC_FILE"
        fi
    fi
}

# Verify installation
verify() {
    if [ -x "${INSTALL_DIR}/${BINARY_NAME}" ]; then
        success "Installation complete!"
        echo ""
        echo "Run 'rat' to start ratterm."
        echo ""
        if ! command -v "$BINARY_NAME" &> /dev/null; then
            warn "Restart your terminal or run: export PATH=\"\$PATH:$INSTALL_DIR\""
        fi
    else
        error "Installation verification failed"
    fi
}

# Uninstall
uninstall() {
    if [ -f "${INSTALL_DIR}/${BINARY_NAME}" ]; then
        rm -f "${INSTALL_DIR}/${BINARY_NAME}"
        success "Uninstalled ${BINARY_NAME}"
    else
        warn "${BINARY_NAME} is not installed"
    fi
    exit 0
}

# Main
main() {
    echo ""
    echo "  ╦═╗╔═╗╔╦╗╔╦╗╔═╗╦═╗╔╦╗"
    echo "  ╠╦╝╠═╣ ║  ║ ║╣ ╠╦╝║║║"
    echo "  ╩╚═╩ ╩ ╩  ╩ ╚═╝╩╚═╩ ╩"
    echo ""

    # Handle uninstall flag
    if [ "$1" = "--uninstall" ] || [ "$1" = "-u" ]; then
        uninstall
    fi

    detect_platform
    get_latest_version
    TEMP_FILE=$(download)
    install_binary "$TEMP_FILE"
    setup_path
    verify
}

main "$@"
