# Idempotency Design

> **Status**: Design approved, ready for implementation
> **Date**: 2025-10-12
> **Related**: RESEARCH_FINDINGS.md, TODO.md

## Overview

Idempotency guarantees ensure that duplicate task/workflow invocations don't cause duplicate side effects. This is critical for:
- Network retries (user clicks submit twice, API timeout retries)
- Worker crashes (task executes but crashes before completion)
- Application logic retries (intentional retry after failure)

## Core Design Decisions

### 1. Task ID (Idempotency Key)

**Optional, user-provided or auto-generated UUID**

```python
# User provides ID (idempotent)
await rhythm.enqueue(
    charge_stripe,
    task_id="payment-order-123"
)

# Auto-generated UUID (no idempotency guarantee)
await rhythm.enqueue(
    send_email,
    user_id=456
    # task_id = "uuid-abc-def-..." auto-generated
)
```

**When to provide task_id**:
- Payment processing
- Critical operations that must not duplicate
- Retry-safe operations

**When to omit task_id**:
- Fire-and-forget tasks
- Idempotent by nature (e.g., setting a value to X)
- Tasks you WANT to run multiple times

### 2. Retention Policy

**Per-queue configuration with sensible defaults**

```toml
# rhythm.toml
[queues.payments]
retention = "30d"  # Keep payment records for 30 days

[queues.emails]
retention = "7d"   # Keep email logs for 7 days

[queues.ephemeral]
retention = "0"    # Delete immediately on completion
```

**Defaults**:
- Tasks: `7 days`
- Workflows: `30 days`

**Retention applies to**:
- Completed tasks
- Failed tasks (for debugging)
- All execution metadata (args, result, timestamps)

### 3. Deduplication Window = Retention Window

**Simplification for v1**: No separate deduplication tracking

**Behavior**:
- `task_id` unique while execution exists in database
- After retention cleanup, `task_id` can be reused
- No separate `deduplication_keys` table in v1

**Example**:
```python
# Day 0: Create task
await rhythm.enqueue(send_report, task_id="daily-report-2025-10-12")

# Day 0-7: Duplicate blocked (task_id exists)
await rhythm.enqueue(send_report, task_id="daily-report-2025-10-12")
# → Returns existing execution

# Day 8: Retention expired, task deleted
await rhythm.enqueue(send_report, task_id="daily-report-2025-10-12")
# → Creates new execution (allowed)
```

### 4. ID Reuse Policy: "Allow Duplicate Failed Only"

**Inspired by Temporal's default behavior**

**Database constraint**:
```sql
CREATE UNIQUE INDEX idx_task_id_active ON executions(task_id)
WHERE task_id IS NOT NULL
  AND status NOT IN ('failed', 'cancelled', 'timed_out');
```

**Behavior matrix**:

| Previous Status | New Enqueue Behavior |
|----------------|---------------------|
| `pending` | ❌ Error: "Task already pending" |
| `running` | ❌ Error: "Task already running" |
| `completed` | ❌ Error: "Task already completed" |
| `failed` | ✅ Creates new execution |
| `cancelled` | ✅ Creates new execution |
| `timed_out` | ✅ Creates new execution |

**Rationale**:
- **Idempotency**: Block duplicate successful executions
- **Retry-ability**: Allow retry after failure
- **Simplicity**: No configuration needed, sensible default

**Hardcoded for v1** - can add configuration later if needed.

### 5. Result Storage

**Automatic based on return value**

```python
@rhythm.task
async def send_email(user_id):
    await email_service.send(user_id)
    # Returns None → result column = NULL

@rhythm.task
async def calculate_tax(order_id):
    return {"tax": 42.50}
    # Returns value → result column = {"tax": 42.50}
```

**No configuration needed** - just works!

**Schema**:
```sql
ALTER TABLE executions ADD COLUMN result JSONB;
```

**Result caching for duplicates**:
```python
# First call
handle1 = await rhythm.enqueue(calculate_tax, task_id="tax-order-123")
result1 = await handle1.result()  # {"tax": 42.50}

# Duplicate call (while first is still pending/running/completed)
handle2 = await rhythm.enqueue(calculate_tax, task_id="tax-order-123")
result2 = await handle2.result()  # Same: {"tax": 42.50} (cached)
```

## Database Schema

### Executions Table Changes

```sql
-- Add task_id column
ALTER TABLE executions ADD COLUMN task_id VARCHAR(255);

-- Add result column
ALTER TABLE executions ADD COLUMN result JSONB;

-- Unique constraint: task_id unique for non-failed statuses
CREATE UNIQUE INDEX idx_task_id_active ON executions(task_id)
WHERE task_id IS NOT NULL
  AND status NOT IN ('failed', 'cancelled', 'timed_out');

-- Index for retention cleanup
CREATE INDEX idx_executions_retention ON executions(completed_at, queue)
WHERE completed_at IS NOT NULL;
```

### Retention Cleanup Query

**Background task runs periodically** (e.g., every hour):

```sql
-- Per-queue retention cleanup
DELETE FROM executions
WHERE completed_at IS NOT NULL
  AND queue = 'emails'
  AND completed_at < NOW() - INTERVAL '7 days';

-- Or batch cleanup across all queues
DELETE FROM executions e
WHERE completed_at IS NOT NULL
  AND completed_at < NOW() - (
    SELECT retention_interval FROM queue_configs qc
    WHERE qc.queue_name = e.queue
  );
```

## Queue Configuration

### Queue-Level Settings

```toml
[queues.payments]
retention = "30d"
rate_limit = "100/sec"

[queues.emails]
retention = "7d"
rate_limit = "500/sec"

[queues.background]
# retention defaults to "7d"
# No rate limit (zero-cost!)
```

**Queues auto-create** - no need to pre-define unless you want custom settings.

### Programmatic Configuration (Future)

```python
# Configure queue in code
rhythm.configure_queue(
    "payments",
    retention=timedelta(days=30),
    rate_limit="100/sec"
)
```

## Task vs Workflow ID Behavior

### Tasks

```python
@rhythm.task(queue="stripe-api")
async def charge_stripe(order_id):
    return {"payment_id": "pay_123"}

# Standalone task
await rhythm.enqueue(
    charge_stripe,
    order_id=456,
    task_id="payment-order-456"  # Optional
)

# Task within workflow
@rhythm.workflow
async def process_order(ctx, order_id):
    result = await ctx.execute_task(
        charge_stripe,
        order_id,
        task_id="payment-order-456"  # Optional
    )
```

**Key point**: Tasks are tasks, whether standalone or called from workflow. No difference!

### Workflows

```python
# Workflow ID always required
await rhythm.start_workflow(
    process_order,
    workflow_id="order-456",  # Required
    order_id=456
)
```

**Workflows always have IDs** because:
- Workflows are long-running (need stable identifier)
- Workflows have state/history (need to reference them)
- Child workflows need to reference parents

## Workflow Steps = Tasks

**No more `@rhythm.task` decorator**

```python
# Define once
@rhythm.task(queue="stripe-api")
async def charge_stripe(order_id):
    pass

# Use as standalone
await rhythm.enqueue(charge_stripe, order_id=123)

# Use as workflow step
@rhythm.workflow
async def process_order(ctx, order_id):
    await ctx.execute_task(charge_stripe, order_id)
```

**Implementation**:
- Tasks have optional `parent_workflow_id` column
- Tasks called from workflows have `parent_workflow_id` set (NULL for standalone)
- But they're just tasks in a queue

## Rate Limiting & Tasks

**Tasks in workflows can be rate-limited via queue config**

```toml
[queues.stripe-api]
rate_limit = "100/sec"  # All Stripe tasks limited, regardless of source
```

```python
@rhythm.task(queue="stripe-api")
async def charge_stripe(order_id):
    pass

# Standalone: rate-limited
await rhythm.enqueue(charge_stripe, order_id=123)

# From workflow: also rate-limited (same queue!)
@rhythm.workflow
async def process_order(ctx, order_id):
    await ctx.execute_task(charge_stripe, order_id)
    # → Goes to "stripe-api" queue → rate limited
```

## Performance Considerations

### Zero-Cost When Not Used

**Rate limiting**:
```python
# Worker startup: determine which queues have rate limits
if queue_config[queue].rate_limit:
    return await claim_with_rate_limit(queue)
else:
    return await claim_direct(queue)  # Fast path
```

**Idempotency**:
```python
# If task_id not provided, no uniqueness check
if task_id:
    # Check for existing task (ON CONFLICT query)
else:
    # Direct insert (fast path)
```

### Retention Cleanup Performance

**Partial index ensures fast claims**:
```sql
CREATE INDEX idx_claimable ON executions(queue, priority, scheduled_at)
WHERE status = 'pending';
```

**Claim query only touches pending tasks** - doesn't scan millions of completed records.

**Cleanup runs in background** - doesn't block claims.

### Optional Redis Layer (Future)

**Phase 2 optimization**:
- Redis cache for task_id deduplication checks (~100x faster)
- PostgreSQL remains source of truth
- Redis failure = fallback to PostgreSQL

**Target performance**:
- Phase 1 (PostgreSQL only): 500-1000 tasks/sec per worker
- Phase 2 (Redis optional): 2000-5000 tasks/sec per worker

## API Examples

### Simple Task (No Idempotency)

```python
@rhythm.task(queue="notifications")
async def send_push_notification(user_id, message):
    await push_service.send(user_id, message)

# Enqueue (no task_id = can run multiple times)
await rhythm.enqueue(send_push_notification, user_id=123, message="Hello")
```

### Idempotent Task

```python
@rhythm.task(queue="payments")
async def charge_customer(order_id, amount):
    return await stripe.charge(order_id, amount)

# Enqueue with task_id (idempotent)
handle = await rhythm.enqueue(
    charge_customer,
    order_id=456,
    amount=99.99,
    task_id=f"charge-order-{order_id}"
)
result = await handle.result()  # {"payment_id": "pay_abc"}

# Duplicate (returns same handle, same result)
handle2 = await rhythm.enqueue(
    charge_customer,
    order_id=456,
    amount=99.99,
    task_id=f"charge-order-{order_id}"
)
result2 = await handle2.result()  # Same: {"payment_id": "pay_abc"}
```

### Workflow with Tasks

```python
@rhythm.task(queue="payments")
async def charge_card(order_id):
    return await stripe.charge(order_id)

@rhythm.task(queue="inventory")
async def reserve_inventory(order_id):
    return await inventory.reserve(order_id)

@rhythm.workflow
async def process_order(ctx, order_id):
    # Workflow steps are tasks with queue routing
    payment = await ctx.execute_task(
        charge_card,
        order_id,
        task_id=f"payment-{order_id}"  # Idempotent
    )

    inventory = await ctx.execute_task(
        reserve_inventory,
        order_id,
        task_id=f"inventory-{order_id}"  # Idempotent
    )

    return {"payment": payment, "inventory": inventory}

# Start workflow
await rhythm.start_workflow(
    process_order,
    workflow_id=f"order-{order_id}",
    order_id=456
)
```

### Retry After Failure

```python
# First attempt
await rhythm.enqueue(
    charge_customer,
    task_id="charge-123",
    order_id=456
)
# → Executes, fails (e.g., network timeout)

# Retry (allowed! status=failed)
await rhythm.enqueue(
    charge_customer,
    task_id="charge-123",
    order_id=456
)
# → Creates new execution with same task_id
```

## Error Messages

### Duplicate Pending/Running

```python
# Task is pending or running
await rhythm.enqueue(charge_card, task_id="payment-123")

# Error response:
{
    "error": "TaskAlreadyExists",
    "message": "Task with ID 'payment-123' is already pending/running",
    "existing_execution_id": 789,
    "status": "running"
}
```

### Duplicate Completed

```python
# Task completed successfully
await rhythm.enqueue(charge_card, task_id="payment-123")
# → Completes

# Duplicate attempt:
{
    "error": "TaskAlreadyCompleted",
    "message": "Task with ID 'payment-123' already completed successfully",
    "existing_execution_id": 789,
    "completed_at": "2025-10-12T14:30:00Z",
    "result": {"payment_id": "pay_abc"}
}
```

## Migration Path

### Phase 1: Core Idempotency (v1)
- ✅ Add `task_id` and `result` columns
- ✅ Implement "Allow Duplicate Failed Only" policy
- ✅ Add retention configuration per queue
- ✅ Background cleanup task
- ✅ Rename task → task throughout codebase

### Phase 2: Optimizations (v2)
- Optional Redis cache for deduplication
- Separate `deduplication_keys` table (for extended dedup windows)
- Configurable ID reuse policies (if users request it)
- Partition executions table by status (hot/cold)

### Phase 3: Advanced (v3)
- Concurrency control (separate from idempotency)
- Per-task concurrency keys
- DBOS-style deduplication scopes

## Implementation Checklist

**Database**:
- [ ] Add `task_id VARCHAR(255)` column to executions
- [ ] Add `result JSONB` column to executions
- [ ] Create unique index on `task_id` (with status filter)
- [ ] Create index for retention cleanup
- [ ] Migration script

**Core**:
- [ ] Rename `task` → `task` in all code
- [ ] Drop `@task` decorator (use `@task` everywhere)
- [ ] Implement task_id generation (UUID if not provided)
- [ ] Implement duplicate detection on enqueue
- [ ] Store result on task completion
- [ ] Return cached result for duplicates

**Configuration**:
- [ ] Add `retention` to queue config
- [ ] Default retention: tasks=7d, workflows=30d
- [ ] Parse retention duration strings ("7d", "30d", etc)

**Cleanup**:
- [ ] Background task for retention cleanup
- [ ] Per-queue cleanup logic
- [ ] Configurable cleanup interval (default: 1 hour)

**API**:
- [ ] `task_id` parameter on `enqueue()`
- [ ] `task_id` parameter on `execute_task()`
- [ ] Return existing execution handle for duplicates
- [ ] Clear error messages for duplicate cases

**Testing**:
- [ ] Test: duplicate pending task blocked
- [ ] Test: duplicate completed task blocked
- [ ] Test: duplicate failed task allowed
- [ ] Test: retention cleanup works
- [ ] Test: result storage and retrieval
- [ ] Test: task_id auto-generation

**Documentation**:
- [ ] Update README with idempotency examples
- [ ] Document when to use task_id
- [ ] Document retention configuration
- [ ] Document ID reuse policy behavior

## Open Questions

None! All decisions made.

## References

- Temporal Workflow ID reuse policies: https://docs.temporal.io/workflow-execution/workflowid-runid
- DBOS idempotency: https://docs.dbos.dev/python/tutorials/workflow-tutorial
- BullMQ deduplication: https://docs.bullmq.io/guide/tasks/deduplication
- RESEARCH_FINDINGS.md: Comprehensive comparison of all platforms
