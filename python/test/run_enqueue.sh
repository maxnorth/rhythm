#!/bin/bash

# Ensure we're in the test directory
cd "$(dirname "$0")"

# Go to python root to find venv
PYTHON_ROOT="$(cd .. && pwd)"

# Activate virtual environment if it exists
if [ -f "$PYTHON_ROOT/.venv/bin/activate" ]; then
    source "$PYTHON_ROOT/.venv/bin/activate"
fi

# Set required environment variables
export CURRANT_DATABASE_URL="postgresql://workflows:workflows@localhost/workflows"
export PYTHONPATH="$PYTHON_ROOT:${PYTHONPATH}"

# Run the test script
echo "Enqueuing tasks with environment:"
echo "  CURRANT_DATABASE_URL=$CURRANT_DATABASE_URL"
echo "  PYTHONPATH=$PYTHONPATH"
echo ""

python test_manual.py
