#!/bin/bash
# Install cc-pulseline for Claude Code
# Builds from source or downloads prebuilt binary

set -euo pipefail

INSTALL_DIR="${HOME}/.claude/pulseline"
BINARY_NAME="cc-pulseline"
REPO="GregoryHo/cc-pulseline"

detect_platform() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Darwin)
            case "$arch" in
                arm64|aarch64) echo "darwin-arm64" ;;
                x86_64)        echo "darwin-x64" ;;
                *)             echo "" ;;
            esac
            ;;
        Linux)
            case "$arch" in
                x86_64)        echo "linux-x64" ;;
                aarch64|arm64) echo "linux-arm64" ;;
                *)             echo "" ;;
            esac
            ;;
        *)
            echo ""
            ;;
    esac
}

download_prebuilt() {
    local platform="$1"
    local url="https://github.com/${REPO}/releases/latest/download/${BINARY_NAME}-${platform}.tar.gz"

    echo "Downloading prebuilt binary for ${platform}..."
    if command -v curl &>/dev/null; then
        curl -fsSL "$url" | tar xz -C "$INSTALL_DIR"
    elif command -v wget &>/dev/null; then
        wget -qO- "$url" | tar xz -C "$INSTALL_DIR"
    else
        echo "Error: curl or wget required for download"
        return 1
    fi
}

build_from_source() {
    echo "Building cc-pulseline from source..."
    cargo build --release 2>&1 || { echo "cargo build failed"; exit 1; }
    cp "target/release/${BINARY_NAME}" "${INSTALL_DIR}/"
}

# Create install directory
mkdir -p "$INSTALL_DIR"

# Try to build from source if cargo is available and we're in the repo
if [ -f "Cargo.toml" ] && command -v cargo &>/dev/null; then
    build_from_source
else
    platform="$(detect_platform)"
    if [ -z "$platform" ]; then
        echo "Error: unsupported platform $(uname -s)/$(uname -m)"
        echo "Please build from source: cargo install cc-pulseline"
        exit 1
    fi
    download_prebuilt "$platform"
fi

chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

# Create default config
"${INSTALL_DIR}/${BINARY_NAME}" --init 2>/dev/null || true

echo ""
echo "Installation complete!"
echo ""
echo "Add to your Claude Code settings (~/.claude/settings.json):"
echo ""
echo '  "statusLine": {'
echo '    "type": "command",'
echo "    \"command\": \"${INSTALL_DIR}/${BINARY_NAME}\""
echo '  }'
echo ""
echo "Commands:"
echo "  ${BINARY_NAME} --help              Show usage"
echo "  ${BINARY_NAME} --init              Create user config"
echo "  ${BINARY_NAME} --init --project    Create project config"
echo "  ${BINARY_NAME} --check             Validate config files"
echo "  ${BINARY_NAME} --print             Show effective merged config"
