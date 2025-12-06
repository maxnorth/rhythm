# Rhythm Python

Python client library for Rhythm.

## Installation

```bash
pip install rhythm-async
```

## Example

```python
import rhythm

# Define a task
@rhythm.task
def send_email(ctx, inputs):
    # Your task implementation
    print("email sent")

# Initialize Rhythm
rhythm.init(
    database_url="postgresql://localhost/rhythm",
    workflow_paths=["./workflows"],
)

# Start a workflow
workflow_id = rhythm.start_workflow(
    "processOrder",
    inputs={"orderId": "123", "amount": 99.99}
)

# Start a worker process (holds the process)
rhythm.start_worker()
```

## Quickstart
```
# environment setup
git clone https://github.com/maxnorth/rhythm.git
docker compose up -d postgres

# start worker
cd python/examples/simple_app
python3 worker.py

# run client app in another terminal
cd python/examples/simple_app
python3 app.py
```

## Documentation

See the [examples](examples/) directory for complete working examples.
