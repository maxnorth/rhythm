# Currant

A lightweight durable execution framework using only Postgres. No external orchestrator needed.

## Features

- **Truly self-contained** - Only depends on Postgres, no external Conductor/orchestrator
- **Durable execution** - Workflows survive crashes and automatically resume
- **Queue-first design** - All work is queued by default
- **Unified platform** - Handle both simple async tasks (Celery-style) and complex workflows (Temporal-style)
- **Transparent replay** - Temporal-style deterministic replay for workflows
- **Worker failover** - Automatic recovery via heartbeat-based coordination through Postgres
- **LISTEN/NOTIFY** - Fast task pickup with Postgres pub/sub
- **Signals** - External systems can send signals to workflows
- **Versioning** - Workflow evolution with backward compatibility

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

### 2. Define Tasks and Currant

```python
# app.py
from currant import task, task, workflow, send_signal, wait_for_signal

# Simple async task
@task(queue="emails", retries=3)
async def send_email(to: str, subject: str, body: str):
    # Your email sending logic
    print(f"Sending email to {to}")
    return {"sent": True}

# Task (workflow step)
@task(retries=3, timeout=60)
async def charge_card(amount: int, card_token: str):
    # Your payment logic
    print(f"Charging ${amount}")
    return {"transaction_id": "txn_123", "amount": amount}

@task()
async def send_receipt(email: str, amount: int):
    print(f"Sending receipt for ${amount} to {email}")

# Workflow (multi-step orchestration)
@workflow(queue="orders", version=1, timeout=3600)
async def process_order(order_id: str, amount: int, email: str, card_token: str):
    # Charge the card (suspends workflow)
    charge_result = await charge_card.run(amount, card_token)

    # Send receipt (suspends workflow)
    await send_receipt.run(email, amount)

    return {"order_id": order_id, "transaction_id": charge_result["transaction_id"]}
```

### 3. Enqueue Work

```python
# client.py
import asyncio
from app import send_email, process_order

async def main():
    # Enqueue a task
    task_id = await send_email.queue(
        to="user@example.com",
        subject="Welcome",
        body="Thanks for signing up!"
    )
    print(f"Task enqueued: {task_id}")

    # Enqueue a workflow
    workflow_id = await process_order.queue(
        order_id="order_123",
        amount=5000,
        email="customer@example.com",
        card_token="tok_visa"
    )
    print(f"Workflow enqueued: {workflow_id}")

asyncio.run(main())
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

## Advanced Features

### Signals

Currant can wait for external signals:

```python
@workflow(queue="approvals", version=1)
async def approval_workflow(document_id: str):
    # Wait for approval signal (suspends workflow)
    approval = await wait_for_signal("approved", timeout=86400)  # 24 hours

    if approval["approved"]:
        await process_document.run(document_id)
        return {"status": "approved"}
    else:
        return {"status": "rejected"}

# Send signal from external system
from currant import send_signal

await send_signal(workflow_id, "approved", {"approved": True, "approver": "user@example.com"})
```

### Workflow Versioning

Handle workflow evolution with backward compatibility:

```python
from currant import get_version

@workflow(queue="orders", version=2)
async def process_order(order_id: str, amount: int, email: str, card_token: str):
    charge_result = await charge_card.run(amount, card_token)

    # Feature added in version 2
    if get_version("send_sms", 1, 2) >= 2:
        await send_sms_notification.run(phone, "Order confirmed!")

    await send_receipt.run(email, amount)
    return {"order_id": order_id}
```

### Dynamic Options

Override execution options at queue time:

```python
# Override queue and priority
task_id = await send_email.options(
    queue="high-priority",
    priority=10
).queue(to="vip@example.com", subject="Urgent", body="...")

# Override timeout for task
@workflow(queue="orders", version=1)
async def risky_order(order_id: str):
    # Give extra time for this charge
    result = await charge_card.options(timeout=120).run(amount, token)
    return result
```

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

### Workflow Replay

Workflows use Temporal-style deterministic replay:

1. Workflow calls `task.run()` → task execution created, workflow suspended
2. Worker picks up task, executes it, stores result
3. Workflow is re-queued with task result in history
4. Worker re-executes workflow function from the beginning
5. Previous Tasks return cached results instantly
6. Workflow continues to next task or completes

This is completely transparent to developers - just write normal async code.

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
| Workflow replay | ✅ Transparent | ✅ Transparent | ✅ Transparent |
| Worker failover | ✅ Via Postgres | ✅ Via Conductor | ✅ Via Server |
| Signals | ✅ Yes | ❌ No | ✅ Yes |
| Versioning | ✅ Yes | ❌ Limited | ✅ Yes |

## License

MIT
