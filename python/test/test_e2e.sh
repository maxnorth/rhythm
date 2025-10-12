#!/bin/bash
set -e

echo "=========================================="
echo "E2E Test: Workflows with Rust Core"
echo "=========================================="
echo

# Activate virtual environment if it exists
if [ -f "$HOME/.venv/bin/activate" ]; then
    source "$HOME/.venv/bin/activate"
fi

# Source cargo environment if it exists
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

# Check prerequisites
if ! command -v cargo &> /dev/null; then
    echo "❌ Error: Rust is not installed"
    echo "   Install from: https://rustup.rs/"
    exit 1
fi

if ! command -v maturin &> /dev/null; then
    echo "Installing maturin..."
    pip install maturin
fi

# Build Rust core
echo "🔨 Building Rust core..."
cd core
maturin develop --release
cd ..
echo "✓ Rust core built"
echo

# Install Python package
echo "📦 Installing Python package..."
pip install -q -e python/
echo "✓ Python package installed"
echo

# Set database URL and Python path
export CURRANT_DATABASE_URL="postgresql://workflows:workflows@localhost/workflows"
export PYTHONPATH="${PWD}:${PYTHONPATH}"
export RUST_LOG=debug

# Check database
echo "🔍 Checking database connection..."
if ! python -c "from workflows.rust_bridge import RustBridge; RustBridge.migrate()" 2>/dev/null; then
    echo "❌ Database connection failed"
    echo "   Make sure PostgreSQL is running:"
    echo "   docker-compose up -d"
    exit 1
fi
echo "✓ Database connected"
echo

# Run migrations
echo "🗄️  Running migrations..."
python -c "from workflows.rust_bridge import RustBridge; RustBridge.migrate()"
echo "✓ Migrations complete"
echo

# Enqueue work
echo "📤 Enqueuing test work..."
python examples/enqueue_example.py
echo

# Start worker in background
echo "👷 Starting worker..."
currant worker -q notifications -q orders -m examples.simple_example &
WORKER_PID=$!

# Wait for worker to process tasks
echo "⏳ Waiting for tasks to complete..."
sleep 5

# Kill worker
kill $WORKER_PID 2>/dev/null || true

echo
echo "=========================================="
echo "✓ E2E Test Complete!"
echo "=========================================="
echo
echo "Check the output above for:"
echo "  - Tasks enqueued successfully"
echo "  - Worker claimed and executed tasks"
echo "  - Workflow steps completed"
