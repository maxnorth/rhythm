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

if [ ! -d "python/.venv" ]; then
    python3 -m venv python/.venv
fi
source python/.venv/bin/activate

# Check for maturin
if ! command -v maturin &> /dev/null; then
    echo "Installing maturin..."
    pip install maturin
fi

# Build and install Python package with Rust extension
echo "Building Python package with Rust extension..."
cd python
maturin develop --release
cd ..

echo "Running migrations..."
export RHYTHM_DATABASE_URL='postgresql://rhythm:rhythm@localhost/rhythm'
python -c 'from rhythm.rust_bridge import RustBridge; RustBridge.migrate()'

echo
echo "✓ Init complete!"
echo
echo "To run examples:"
echo "  python examples/enqueue_example.py"
echo "  python -m rhythm worker -q orders -q notifications -m examples.simple_example"
