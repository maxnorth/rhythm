# Currant

A lightweight durable execution framework using only Postgres. No external orchestrator needed.

## Features

- **Truly self-contained** - Only depends on Postgres, no external Conductor/orchestrator
- **Durable execution** - Workflows survive crashes and automatically resume
- **DSL-based workflows** - Simple `.flow` files for language-agnostic orchestration
- **Queue-first design** - All work is queued by default
- **Unified platform** - Handle both simple async tasks (Celery-style) and complex workflows
- **Worker failover** - Automatic recovery via heartbeat-based coordination through Postgres
- **LISTEN/NOTIFY** - Fast task pickup with Postgres pub/sub
- **Signals** - External systems can send signals to workflows (Python workflows)
- **Versioning** - Workflow evolution with backward compatibility (Python workflows)

## Installation

```bash
pip install -e .
```

## Quick Start

### 1. Setup Database

```bash
# Set database URL
export CURRANT_DATABASE_URL="postgresql://localhost/currant"

# Run migrations
currant migrate
```

### 2. Define Tasks and Workflows

```python
# app.py - Define tasks
import currant
from currant import task

# Initialize with workflow paths
currant.init(
    database_url="postgresql://localhost/currant",
    workflow_paths=["./workflows"]
)

# Define tasks that workflows can call
@task(queue="payments")
async def charge_card(order_id: str, amount: float):
    print(f"💳 Charging ${amount} for order {order_id}")
    return {"success": True, "transaction_id": "txn_123"}

@task(queue="fulfillment")
async def ship_order(order_id: str):
    print(f"📦 Shipping order {order_id}")
    return {"success": True, "tracking": "TRACK123"}

@task(queue="emails")
async def send_email(to: str, subject: str, body: str):
    print(f"📧 Sending email to {to}")
    return {"sent": True}
```

```
// workflows/processOrder.flow - Define workflow
task("charge_card", { "order_id": "order-123", "amount": 99.99 })
task("ship_order", { "order_id": "order-123" })
task("send_email", { "to": "customer@example.com", "subject": "Order shipped!" })
```

### 3. Start Workflows

```python
import currant

# Start a DSL workflow
workflow_id = await currant.start_workflow(
    "processOrder",
    inputs={"orderId": "order-123", "amount": 99.99}
)
print(f"Workflow started: {workflow_id}")
```

### 4. Run Workers

```bash
# Start worker for emails queue
currant worker -q emails

# Start worker for orders queue
currant worker -q orders

# Start worker for multiple queues
currant worker -q emails -q orders
```

## Workflow Types

### DSL Workflows (Recommended)

Language-agnostic workflows defined in `.flow` files:

**Benefits:**
- Same workflow works with Python, Node.js, or any language
- Simple flat state (no complex replay)
- Easier to visualize and debug
- Inherently deterministic

**Current syntax:**
```
task("taskName", { "arg": "value" })
sleep(5)
```

**Coming soon:**
- Conditionals: `if (result.success) { ... }`
- Loops: `for (item in items) { ... }`
- Expressions: Variables, operators, property access

### Task Options

**Dynamic Options** - Override execution options at queue time:
```python
# Override queue and priority for tasks
task_id = await send_email.options(
    queue="high-priority",
    priority=10
).queue(to="vip@example.com", subject="Urgent", body="...")

## CLI Commands

```bash
# Run migrations
currant migrate

# Start worker
currant worker -q queue_name

# Check execution status
currant status <execution_id>

# List executions
currant list
currant list --queue emails --status pending
currant list --limit 50

# Cancel execution
currant cancel <execution_id>
```

## Configuration

Set via environment variables (prefix with `CURRANT_`):

```bash
# Database
export CURRANT_DATABASE_URL="postgresql://localhost/currant"

# Worker settings
export CURRANT_WORKER_HEARTBEAT_INTERVAL=5  # seconds
export CURRANT_WORKER_HEARTBEAT_TIMEOUT=30  # seconds
export CURRANT_WORKER_POLL_INTERVAL=1  # seconds
export CURRANT_WORKER_MAX_CONCURRENT=10  # per worker

# Execution defaults
export CURRANT_DEFAULT_TIMEOUT=300  # seconds
export CURRANT_DEFAULT_WORKFLOW_TIMEOUT=3600  # seconds
export CURRANT_DEFAULT_RETRIES=3
```

## How It Works

### Worker Coordination (No External Orchestrator!)

Unlike DBOS which requires a separate Conductor service, currant achieves worker failover entirely through Postgres:

1. **Heartbeats** - Workers update a heartbeat table every 5s
2. **Dead worker detection** - Workers detect when other workers haven't heartbeat in 30s
3. **Work recovery** - Dead workers' executions are reset to pending and re-queued
4. **LISTEN/NOTIFY** - Workers listen for new work via Postgres pub/sub for instant pickup

### Workflow Execution

DSL workflows use simple state persistence:

1. Parse `.flow` file to AST, store in database
2. Execute statement by statement (tree-walking interpreter)
3. On `task()`: Create child execution, save state `{statement_index, locals}`, suspend
4. When child completes: Resume workflow, continue from next statement
5. No replay needed - just continue from saved position

## Architecture

```
┌─────────────┐
│  Client     │ - Enqueues tasks/workflows
└──────┬──────┘
       │
       v
┌─────────────────────────────────────┐
│         Postgres Database           │
│  • executions (tasks/workflows)     │
│  • worker_heartbeats                │
│  • workflow_signals                 │
│  • LISTEN/NOTIFY channels           │
└─────────────┬───────────────────────┘
              │
       ┌──────┴──────┐
       │             │
       v             v
┌──────────┐  ┌──────────┐
│ Worker 1 │  │ Worker 2 │ - Poll for work
│          │  │          │ - Execute functions
│          │  │          │ - Heartbeat
│          │  │          │ - Detect failures
└──────────┘  └──────────┘
```

## Comparison

| Feature | Currant | DBOS Transact | Temporal |
|---------|-----------|---------------|----------|
| External orchestrator | ❌ None | ✅ Conductor required | ✅ Server required |
| Database | Postgres only | Postgres only | Any (via adapter) |
| Queue-first | ✅ Yes | ❌ Sync by default | ✅ Yes |
| Workflow style | DSL-based | Python/TS code | Language code |
| Language-agnostic workflows | ✅ Yes | ❌ No | ❌ No |
| Worker failover | ✅ Via Postgres | ✅ Via Conductor | ✅ Via Server |
| Signals | 🚧 Planned | ❌ No | ✅ Yes |
| Versioning | 🚧 Planned | ❌ Limited | ✅ Yes |

## License

MIT
