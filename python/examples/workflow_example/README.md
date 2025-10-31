# Rhythm Workflow Example

This example demonstrates how to use Rhythm's DSL-based workflows.

## Project Structure

```
workflow_example/
├── workflows/              # Workflow definitions (.flow files)
│   ├── processOrder.flow
│   └── sendDailyReport.flow
├── main.py                 # Application code with task definitions
└── README.md
```

## How It Works

### 1. Workflow Files (.flow)

Workflows are written in a simple DSL and stored as `.flow` files:

```javascript
// workflows/processOrder.flow
task("chargeCard", { "orderId": "order-123", "amount": 99.99 })
sleep(5)
task("shipOrder", { "orderId": "order-123" })
task("sendEmail", { "to": "customer@example.com", "subject": "Order shipped!", "body": "Your order is on the way" })
```

### 2. Task Definitions (Python)

Tasks are Python functions decorated with `@rhythm.task`:

```python
@rhythm.task(name="chargeCard", queue="payments")
async def charge_card(order_id: str, amount: float):
    # Business logic here
    return {"success": True, "transaction_id": "tx_123456"}
```

### 3. Initialization

At startup, initialize Rhythm with workflow paths:

```python
rhythm.init(
    database_url="postgresql://rhythm@localhost/rhythm",
    workflow_paths=["./workflows"]
)
```

This scans the `workflows/` directory, parses all `.flow` files, and stores them in the database.

### 4. Starting Workflows

Queue a workflow execution:

```python
workflow_id = await rhythm.start_workflow(
    "processOrder",
    inputs={"orderId": "order-123", "amount": 99.99}
)
```

## Running the Example

```bash
# Start database
docker compose up -d

# Run migrations
cd python
.venv/bin/python -m rhythm migrate

# Run the example
cd examples/workflow_example
python main.py
```

## Key Concepts

- **Workflows** = Orchestration logic in `.flow` files
- **Tasks** = Business logic in Python functions
- **Separation of Concerns** = Complex logic stays in Python, workflows just coordinate
- **Versioning** = Each workflow execution stores its own copy of the workflow definition
- **Durable Execution** = Workflows can pause/resume across restarts
