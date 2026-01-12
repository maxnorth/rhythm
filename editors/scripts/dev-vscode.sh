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
#   ./editors/scripts/dev-vscode.sh
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"

echo "=== Building rhythm-lsp (debug) ==="
cd "$PROJECT_ROOT/editors/lsp"
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
echo "=== Creating symlink for local development ==="
EXTENSIONS_DIR="$HOME/.vscode/extensions"
SYMLINK_PATH="$EXTENSIONS_DIR/rhythm-lang"

if [ -L "$SYMLINK_PATH" ]; then
    echo "Symlink already exists at $SYMLINK_PATH"
elif [ -e "$SYMLINK_PATH" ]; then
    echo "Warning: $SYMLINK_PATH exists but is not a symlink. Skipping."
else
    mkdir -p "$EXTENSIONS_DIR"
    ln -s "$PROJECT_ROOT/editors/vscode" "$SYMLINK_PATH"
    echo "Created symlink: $SYMLINK_PATH -> $PROJECT_ROOT/editors/vscode"
fi

echo ""
echo "=== Setup complete! ==="
echo ""
echo "The extension is now installed in your VS Code."
echo "Restart VS Code or run 'Developer: Reload Window' to load it."
echo ""
echo "After making changes:"
echo "  - LSP changes: Run 'cargo build' in editors/lsp/, then 'Rhythm: Restart Language Server'"
echo "  - Extension changes: Run 'npm run compile' in editors/vscode/, then reload VS Code"
echo ""
echo "The extension uses the debug build of rhythm-lsp at:"
echo "  $PROJECT_ROOT/editors/lsp/target/debug/rhythm-lsp"
