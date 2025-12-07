#!/bin/bash
# Ratterm Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/hastur-dev/ratterm/main/install.sh | bash
# Debug: curl -fsSL https://raw.githubusercontent.com/hastur-dev/ratterm/main/install.sh | bash -s -- --verbose

set -e

VERSION="0.1.0"
REPO="hastur-dev/ratterm"
BINARY_NAME="rat"
INSTALL_DIR="${RATTERM_INSTALL_DIR:-$HOME/.local/bin}"
VERBOSE="${VERBOSE:-false}"

# Parse arguments
for arg in "$@"; do
    case $arg in
        --verbose|-v)
            VERBOSE=true
            ;;
        --uninstall|-u)
            UNINSTALL=true
            ;;
    esac
done

# All output functions write to stderr to avoid polluting stdout
# Note: Colors removed to prevent issues with command substitution
info() { echo "[INFO] $1" >&2; }
success() { echo "[SUCCESS] $1" >&2; }
warn() { echo "[WARN] $1" >&2; }
error() { echo "[ERROR] $1" >&2; exit 1; }
debug() {
    if [ "$VERBOSE" = true ]; then
        echo "[DEBUG] $1" >&2
    fi
}

# Log system information for debugging
log_system_info() {
    debug "=== System Information ==="
    debug "Date: $(date)"
    debug "Shell: $SHELL"
    debug "Bash version: ${BASH_VERSION:-unknown}"
    debug "User: $(whoami)"
    debug "Home: $HOME"
    debug "PWD: $(pwd)"
    debug "PATH: $PATH"
    debug "Install dir: $INSTALL_DIR"

    if command -v uname &> /dev/null; then
        debug "OS: $(uname -a)"
    fi

    if command -v curl &> /dev/null; then
        debug "curl version: $(curl --version | head -1)"
    else
        debug "curl: not found"
    fi

    if command -v wget &> /dev/null; then
        debug "wget version: $(wget --version | head -1)"
    else
        debug "wget: not found"
    fi

    debug "=== End System Information ==="
}

# Detect OS and architecture
detect_platform() {
    debug "Detecting platform..."

    OS="$(uname -s)"
    ARCH="$(uname -m)"

    debug "Raw OS: $OS"
    debug "Raw ARCH: $ARCH"

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

    debug "Detected OS_NAME: $OS_NAME"
    debug "Detected ARCH_NAME: $ARCH_NAME"
    debug "Asset name: $ASSET_NAME"

    info "Detected platform: $OS_NAME-$ARCH_NAME"
}

# Get latest version from GitHub
get_latest_version() {
    debug "Fetching latest version from GitHub API..."

    local api_url="https://api.github.com/repos/${REPO}/releases/latest"
    debug "API URL: $api_url"

    if command -v curl &> /dev/null; then
        debug "Using curl to fetch version..."
        local response
        response=$(curl -fsSL "$api_url" 2>&1) || {
            debug "curl failed with exit code: $?"
            debug "Response: $response"
            warn "Could not fetch latest version from API"
            return
        }
        debug "API response received (${#response} bytes)"
        LATEST=$(echo "$response" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')
    elif command -v wget &> /dev/null; then
        debug "Using wget to fetch version..."
        local response
        response=$(wget -qO- "$api_url" 2>&1) || {
            debug "wget failed with exit code: $?"
            debug "Response: $response"
            warn "Could not fetch latest version from API"
            return
        }
        debug "API response received (${#response} bytes)"
        LATEST=$(echo "$response" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')
    else
        error "Neither curl nor wget found. Please install one of them."
    fi

    debug "Parsed version: '$LATEST'"

    if [ -n "$LATEST" ] && [ "$LATEST" != "" ]; then
        VERSION="$LATEST"
        info "Latest version: v$VERSION"
    else
        warn "Could not parse latest version, using default: v$VERSION"
    fi
}

# Create a safe temp file (portable, doesn't rely on mktemp)
create_temp_file() {
    local template="${TMPDIR:-/tmp}/ratterm.XXXXXX"
    local temp_file=""

    # Try mktemp first (most common)
    if command -v mktemp &> /dev/null; then
        temp_file=$(mktemp 2>/dev/null) || temp_file=""
    fi

    # Fallback: create temp file manually with random suffix
    if [ -z "$temp_file" ] || [ ! -f "$temp_file" ]; then
        local tmpdir="${TMPDIR:-/tmp}"
        local random_suffix="$$.$RANDOM.$(date +%s)"
        temp_file="${tmpdir}/ratterm-download-${random_suffix}"
        # Create the file securely (fail if exists)
        (umask 077 && : > "$temp_file") || {
            error "Failed to create temp file: $temp_file"
        }
    fi

    echo "$temp_file"
}

# Download binary
download() {
    local download_url="https://github.com/${REPO}/releases/download/v${VERSION}/${ASSET_NAME}"
    local temp_file
    temp_file=$(create_temp_file)

    debug "Download URL: $download_url"
    debug "Temp file: $temp_file"

    info "Downloading ${BINARY_NAME} v${VERSION}..."

    if command -v curl &> /dev/null; then
        debug "Using curl to download..."
        local http_code
        http_code=$(curl -fsSL -w "%{http_code}" "$download_url" -o "$temp_file" 2>&1) || {
            local exit_code=$?
            debug "curl failed with exit code: $exit_code"
            debug "HTTP code: $http_code"
            rm -f "$temp_file"
            error "Download failed (exit code: $exit_code). Check if release v${VERSION} exists at: $download_url"
        }
        debug "Download completed with HTTP code: $http_code"
    elif command -v wget &> /dev/null; then
        debug "Using wget to download..."
        wget -q "$download_url" -O "$temp_file" 2>&1 || {
            local exit_code=$?
            debug "wget failed with exit code: $exit_code"
            rm -f "$temp_file"
            error "Download failed (exit code: $exit_code). Check if release v${VERSION} exists at: $download_url"
        }
        debug "Download completed"
    fi

    # Verify download
    if [ ! -f "$temp_file" ]; then
        error "Download failed: temp file does not exist"
    fi

    local file_size
    file_size=$(wc -c < "$temp_file" 2>/dev/null || echo "0")
    debug "Downloaded file size: $file_size bytes"

    if [ "$file_size" -lt 1000 ]; then
        debug "File contents (might be error message):"
        debug "$(cat "$temp_file" 2>/dev/null || echo 'unable to read')"
        rm -f "$temp_file"
        error "Download failed: file too small ($file_size bytes), likely an error page"
    fi

    # Return only the temp file path
    echo "$temp_file"
}

# Install binary
install_binary() {
    local temp_file="$1"

    debug "Installing binary from: $temp_file"
    debug "Install directory: $INSTALL_DIR"

    # Create install directory
    if [ ! -d "$INSTALL_DIR" ]; then
        debug "Creating install directory: $INSTALL_DIR"
        mkdir -p "$INSTALL_DIR" || error "Failed to create install directory: $INSTALL_DIR"
    fi

    # Determine destination path
    if [ "$OS_NAME" = "windows" ]; then
        DEST="${INSTALL_DIR}/${BINARY_NAME}.exe"
    else
        DEST="${INSTALL_DIR}/${BINARY_NAME}"
    fi

    debug "Destination: $DEST"

    # Move binary
    debug "Moving $temp_file to $DEST"
    mv "$temp_file" "$DEST" || error "Failed to move binary to $DEST"

    # Make executable
    debug "Setting executable permissions"
    chmod +x "$DEST" || error "Failed to set executable permissions on $DEST"

    # Verify installation
    if [ -x "$DEST" ]; then
        debug "Binary installed and executable"
        success "Installed ${BINARY_NAME} to ${DEST}"
    else
        error "Installation verification failed: $DEST is not executable"
    fi
}

# Add to PATH
setup_path() {
    debug "Setting up PATH..."

    # Check if already in PATH
    if echo "$PATH" | grep -q "$INSTALL_DIR"; then
        debug "Install directory already in PATH"
        return
    fi

    SHELL_NAME=$(basename "$SHELL")
    debug "Shell: $SHELL_NAME"

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
            debug "Unknown shell, cannot auto-configure PATH"
            ;;
    esac

    if [ -n "$RC_FILE" ]; then
        debug "RC file: $RC_FILE"
        if ! grep -q "$INSTALL_DIR" "$RC_FILE" 2>/dev/null; then
            debug "Adding PATH entry to $RC_FILE"
            {
                echo ""
                echo "# Ratterm"
                if [ "$SHELL_NAME" = "fish" ]; then
                    echo "set -gx PATH \$PATH $INSTALL_DIR"
                else
                    echo "export PATH=\"\$PATH:$INSTALL_DIR\""
                fi
            } >> "$RC_FILE"
            info "Added $INSTALL_DIR to PATH in $RC_FILE"
        else
            debug "PATH entry already exists in $RC_FILE"
        fi
    fi
}

# Verify installation
verify() {
    debug "Verifying installation..."

    if [ -x "${INSTALL_DIR}/${BINARY_NAME}" ]; then
        local version_output
        version_output=$("${INSTALL_DIR}/${BINARY_NAME}" --version 2>&1 || echo "version check failed")
        debug "Version output: $version_output"

        success "Installation complete!"
        echo ""
        echo "Run 'rat' to start ratterm."
        echo ""
        if ! command -v "$BINARY_NAME" &> /dev/null; then
            warn "Restart your terminal or run: export PATH=\"\$PATH:$INSTALL_DIR\""
        fi
    else
        error "Installation verification failed: ${INSTALL_DIR}/${BINARY_NAME} not found or not executable"
    fi
}

# Uninstall
uninstall() {
    info "Uninstalling ${BINARY_NAME}..."

    if [ -f "${INSTALL_DIR}/${BINARY_NAME}" ]; then
        debug "Removing ${INSTALL_DIR}/${BINARY_NAME}"
        rm -f "${INSTALL_DIR}/${BINARY_NAME}"
        success "Uninstalled ${BINARY_NAME}"
    else
        warn "${BINARY_NAME} is not installed at ${INSTALL_DIR}"
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

    # Log system info if verbose
    if [ "$VERBOSE" = true ]; then
        info "Verbose mode enabled"
        log_system_info
    fi

    # Handle uninstall flag
    if [ "$UNINSTALL" = true ]; then
        uninstall
    fi

    detect_platform
    get_latest_version

    debug "Starting download..."
    TEMP_FILE=$(download)
    debug "Download returned: '$TEMP_FILE'"

    if [ -z "$TEMP_FILE" ] || [ ! -f "$TEMP_FILE" ]; then
        error "Download failed: no temp file returned"
    fi

    install_binary "$TEMP_FILE"
    setup_path
    verify
}

main "$@"
