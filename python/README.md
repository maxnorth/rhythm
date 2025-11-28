# Rhythm Python

Python client library for Rhythm - a lightweight durable execution framework.

## Installation

### Development Install

From the `python` directory:

```bash
pip install -e .
```

This will:
1. Build the Rust extension (rhythm_core) using maturin
2. Install the Python package in development mode

### Production Install

```bash
pip install rhythm
```

## Quick Start

```python
import rhythm
from rhythm import task

# Define a task
@task
async def send_email(to: str, subject: str, body: str):
    # Your task implementation
    pass

# Initialize Rhythm
rhythm.init(
    database_url="postgresql://localhost/rhythm",
    workflow_paths=["./workflows"],
)

# Start a workflow
workflow_id = await rhythm.start_workflow(
    "processOrder",
    inputs={"orderId": "123", "amount": 99.99}
)
```

## Documentation

See the [examples](examples/) directory for complete working examples.
