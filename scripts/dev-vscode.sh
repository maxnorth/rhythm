#!/usr/bin/env bash
#
# Set up VS Code extension for local development
#
# This script:
# 1. Builds the LSP in debug mode
# 2. Builds the VS Code extension
# 3. Creates a symlink so VS Code can use the local extension
#
# Usage:
#   ./scripts/dev-vscode.sh
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== Building rhythm-lsp (debug) ==="
cd "$PROJECT_ROOT/lsp"
cargo build

echo ""
echo "=== Building VS Code extension ==="
cd "$PROJECT_ROOT/editors/vscode"

# Install dependencies if needed
if [ ! -d "node_modules" ]; then
    echo "Installing npm dependencies..."
    npm install
fi

# Compile TypeScript
npm run compile

echo ""
echo "=== Setup complete! ==="
echo ""
echo "To test the extension in VS Code:"
echo ""
echo "  1. Open VS Code in the extension directory:"
echo "     code $PROJECT_ROOT/editors/vscode"
echo ""
echo "  2. Press F5 to launch a new VS Code window with the extension loaded"
echo ""
echo "  3. Open a .flow file to test the extension"
echo ""
echo "The extension will use the debug build of rhythm-lsp at:"
echo "  $PROJECT_ROOT/lsp/target/debug/rhythm-lsp"
