#!/bin/bash
# Quick install for cc-pulseline
# Builds from source, installs to ~/.local/bin, creates default config

set -euo pipefail

INSTALL_DIR="${HOME}/.local/bin"
BINARY_NAME="cc-pulseline"

echo "Building cc-pulseline..."
cargo build --release 2>&1 || { echo "cargo build failed"; exit 1; }

echo "Installing to ${INSTALL_DIR}..."
mkdir -p "$INSTALL_DIR"
cp "target/release/${BINARY_NAME}" "${INSTALL_DIR}/"
chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

echo "Creating default config..."
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
echo "Make sure ${INSTALL_DIR} is in your PATH:"
echo "  export PATH=\"\${HOME}/.local/bin:\${PATH}\""
echo ""
echo "Commands:"
echo "  ${BINARY_NAME} --init              Create user config"
echo "  ${BINARY_NAME} --init --project    Create project config"
echo "  ${BINARY_NAME} --check             Validate config files"
echo "  ${BINARY_NAME} --print             Show effective merged config"
