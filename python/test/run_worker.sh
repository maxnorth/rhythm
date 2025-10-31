#!/bin/bash

# Ensure we're in the project directory
cd "$(dirname "$0")/.."

# Activate virtual environment if it exists
if [ -f ".venv/bin/activate" ]; then
    source ".venv/bin/activate"
fi

# Set required environment variables
export RHYTHM_DATABASE_URL="postgresql://workflows:workflows@localhost/workflows"
export PYTHONPATH="${PWD}:${PYTHONPATH}"

# Run the worker
echo "Starting worker with environment:"
echo "  RHYTHM_DATABASE_URL=$RHYTHM_DATABASE_URL"
echo "  PYTHONPATH=$PYTHONPATH"
echo ""

rhythm worker -q notifications -q orders -m examples.simple_example
