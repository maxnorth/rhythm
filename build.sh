#!/bin/bash
set -e

echo "Building Workflows project..."
echo

# Source cargo environment if it exists
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust is not installed."
    echo "Install from: https://rustup.rs/"
    exit 1
fi

# Check for maturin
if ! command -v maturin &> /dev/null; then
    echo "Installing maturin..."
    pip install maturin
fi

# Build Rust core and install Python extension
echo "Building Rust core..."
cd core
maturin develop --release
cd ..

# Install Python package
echo "Installing Python package..."
pip install -e python/

echo
echo "✓ Build complete!"
echo
echo "To run migrations:"
echo "  export CURRANT_DATABASE_URL='postgresql://localhost/workflows'"
echo "  python -c 'from workflows.rust_bridge import RustBridge; RustBridge.migrate()'"
echo
echo "To run examples:"
echo "  python examples/enqueue_example.py"
echo "  python -m currant worker -q orders -q notifications -m examples.simple_example"
