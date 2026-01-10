#!/usr/bin/env bash
#
# Install rhythm-lsp locally
#
# Usage:
#   ./scripts/install-lsp.sh           # Install to ~/.local/bin
#   ./scripts/install-lsp.sh /usr/local/bin  # Install to custom location
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Default install location
INSTALL_DIR="${1:-$HOME/.local/bin}"

echo "Building rhythm-lsp..."
cd "$PROJECT_ROOT/lsp"
cargo build --release

echo "Installing to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
cp "$PROJECT_ROOT/lsp/target/release/rhythm-lsp" "$INSTALL_DIR/"

echo "Done! rhythm-lsp installed to $INSTALL_DIR/rhythm-lsp"

# Check if the install directory is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo "Note: $INSTALL_DIR is not in your PATH."
    echo "Add the following to your shell profile:"
    echo ""
    echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
fi
