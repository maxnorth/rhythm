#!/bin/bash

# Ensure we're in the project directory
cd "$(dirname "$0")"

# Activate virtual environment if it exists
if [ -f "$HOME/.venv/bin/activate" ]; then
    source "$HOME/.venv/bin/activate"
fi

# Set required environment variables
export WORKFLOWS_DATABASE_URL="postgresql://workflows:workflows@localhost/workflows"
export PYTHONPATH="${PWD}:${PYTHONPATH}"

# Run the test script
echo "Enqueuing jobs with environment:"
echo "  WORKFLOWS_DATABASE_URL=$WORKFLOWS_DATABASE_URL"
echo "  PYTHONPATH=$PYTHONPATH"
echo ""

python test_manual.py
