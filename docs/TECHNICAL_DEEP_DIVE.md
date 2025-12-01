# Technical Deep Dive

This document explains how Rhythm's snapshot-based execution model works under the hood.

---

## Snapshot Model

### What Gets Snapshotted

When a workflow hits an `await` statement, Rhythm saves:

- **Current statement index** - Where execution paused
- **Local variables** - All workflow-local state
- **Awaited task ID** - The task we're waiting for

Snapshots are stored as JSON in PostgreSQL's `workflow_execution_context` table.

### On Resume

When a workflow resumes (after the awaited task completes):

1. Load snapshot from Postgres
2. Restore local variables
3. Continue from next statement
4. **No replay**, no re-execution of past code

### Example

```javascript
// workflow.flow
workflow(ctx, inputs) {
  let payment = await task("chargeCard", { amount: 100 })
  // ^ Snapshot happens here
  //   Saved: { statement_index: 1, locals: { payment: null }, awaiting_task_id: "task_123" }

  await task("shipOrder", { orderId: payment.orderId })
  // ^ Snapshot happens here
  //   Saved: { statement_index: 2, locals: { payment: {...} }, awaiting_task_id: "task_456" }
}
```

**What doesn't get snapshotted**: Python/JS stack frames. Only DSL execution state is persisted.

---

## Versioning

### How It Works

Each workflow's source code is hashed (SHA-256). The hash becomes the version identifier.

- **Old workflows**: Keep running on their version indefinitely
- **New deployments**: Start workflows use the new version hash
- **No automatic migration**: Old in-flight workflows finish on old version

### Example

```
v1 (hash: abc123): workflow { task("foo") }
v2 (hash: def456): workflow { task("foo"); task("bar") }

Deployment timeline:
- 100 workflows running on v1
- Deploy v2
- New starts use v2
- Old 100 workflows continue using v1 (even if it takes days)
```

### Migration Strategy

If you need to migrate in-flight workflows:

1. Pause old workflows (mark as suspended)
2. Inspect their state
3. Manually create new executions on new version with equivalent state
4. Cancel old executions

**Trade-off**: Less automatic than Temporal's replay-based migration, but simpler mental model.

---

## Failure Modes

### Snapshot Schema Changes

If the DSL interpreter's snapshot format changes (rare), old workflows may not be able to resume.

**Mitigation**:
- Version the snapshot schema itself
- Maintain backward compatibility in the interpreter
- Manual intervention if truly breaking

**Example breaking change**: Adding required fields to snapshot format without defaults.

### Worker Crashes

When a worker crashes mid-execution:

1. Heartbeat stops updating
2. Other workers detect dead worker (30s timeout)
3. Work is reset to `pending` status
4. Another worker claims and resumes from last snapshot

**Idempotency**: Tasks should be idempotent since they may execute multiple times if workers crash during execution.

---

## Architecture

### Component Layout

```
┌─────────────────────────────┐
│  Workflow DSL (.flow files) │
│  Parsed by Pest grammar      │
└─────────────┬───────────────┘
              │
              v
┌─────────────────────────────┐
│     Rust Core Engine         │
│  • Tree-walking interpreter  │
│  • Snapshot serialization    │
│  • Worker coordination       │
│  • Task execution            │
└─────────────┬───────────────┘
              │
              v
┌─────────────────────────────┐
│      PostgreSQL Only         │
│  • workflow_definitions      │
│  • workflow_execution_context│
│  • executions (task queue)   │
│  • worker_heartbeats         │
└─────────────────────────────┘
```

### Language Adapters

**Rust Core** exposes FFI functions via:
- **PyO3** for Python
- **NAPI-RS** for Node.js

**Adapters provide**:
- Task decorators (`@task` in Python, `task()` in JS)
- Worker loops (claim work, execute, report results)
- Client APIs (`start_workflow()`, `queue()`)

**Universal execution**: Same workflow DSL runs everywhere. Only task implementations are language-specific.

---

## Worker Coordination

### Heartbeat-Based Failover

**No external orchestrator**. Workers coordinate via Postgres.

**Every 5 seconds**, each worker:
```sql
INSERT INTO worker_heartbeats (worker_id, last_heartbeat)
VALUES ('worker-1', NOW())
ON CONFLICT (worker_id) UPDATE SET last_heartbeat = NOW()
```

**Every 30 seconds**, each worker checks:
```sql
SELECT worker_id FROM worker_heartbeats
WHERE last_heartbeat < NOW() - INTERVAL '30 seconds'
```

**If dead workers found**:
```sql
UPDATE executions
SET status = 'pending', worker_id = NULL
WHERE worker_id IN (dead_workers)
  AND status IN ('running', 'suspended')
```

**Work is automatically recovered** without manual intervention.

---

## Task Claiming

Workers poll for work using Postgres row-level locking:

```sql
SELECT * FROM executions
WHERE status = 'pending'
  AND queue = 'orders'
  AND scheduled_at <= NOW()
ORDER BY priority DESC, created_at ASC
LIMIT 1
FOR UPDATE SKIP LOCKED
```

**`SKIP LOCKED`** prevents workers from blocking each other. Each worker gets a different task.

**Polling interval**: Default 1 second (configurable via `RHYTHM_WORKER_POLL_INTERVAL`).

---

## Performance Characteristics

### Current State

**No comprehensive benchmarks yet.** Early testing shows:
- ~27 tasks/sec with 5 workers (pre-optimization)
- Bottleneck: Database connection pool saturation

**Expected optimizations**:
- Connection pool tuning
- Batch task claiming
- LISTEN/NOTIFY for instant wakeup (reduces polling overhead)

**Target**: 1000s of tasks/sec.

### Postgres-Only Trade-offs

**Advantages**:
- One database vs database + Redis + queue system
- Transactional guarantees (tasks + snapshots in same transaction)
- Simple operational model

**Limitations**:
- Lower throughput ceiling than Redis-based queues
- Unknown scaling beyond single Postgres instance
- Lock contention under high concurrency (not yet tested)

**Good for**: Most applications (<100 workers, moderate throughput)
**Not for**: Extreme scale (100k+ tasks/sec), multi-region distribution

---

## Exactly-Once Semantics

Tasks and snapshots commit in the same Postgres transaction (outbox pattern built-in).

**Example**:
```python
@rhythm.task(queue="orders")
async def charge_card(order_id: str, amount: float):
    async with db.transaction():
        # Create charge record
        charge = await db.charges.create(order_id=order_id, amount=amount)

        # Return result (will be committed with snapshot)
        return {"charge_id": charge.id}
```

**If worker crashes** after executing but before committing:
- Task execution rolls back
- Snapshot doesn't update
- Another worker will retry the task

**Idempotency keys** (planned) will deduplicate retries at the application level.

---

## Snapshot Size and Serialization

### Current Implementation

- **Serialization**: JSON via `serde_json`
- **Typical size**: <1KB per snapshot (statement index + locals)
- **Storage**: `workflow_execution_context.state` column (JSONB)

### Limitations

**Large state**: If workflows accumulate large objects in locals, snapshots grow.

**Mitigation** (planned):
- Compression (gzip)
- External blob storage for large state
- Warnings when snapshots exceed threshold

**No performance testing yet** on large state or high-frequency snapshots.

---

## Database Schema

### Core Tables

**`executions`** - Task and workflow queue
```sql
CREATE TABLE executions (
    id UUID PRIMARY KEY,
    execution_type VARCHAR NOT NULL, -- 'task' or 'workflow'
    target_name VARCHAR NOT NULL,
    queue VARCHAR NOT NULL,
    status VARCHAR NOT NULL, -- 'pending', 'running', 'completed', 'failed', 'suspended'
    inputs JSONB,
    result JSONB,
    worker_id VARCHAR,
    priority INTEGER DEFAULT 0,
    scheduled_at TIMESTAMP,
    created_at TIMESTAMP,
    updated_at TIMESTAMP
)
```

**`workflow_execution_context`** - Snapshot storage
```sql
CREATE TABLE workflow_execution_context (
    execution_id UUID PRIMARY KEY REFERENCES executions(id),
    workflow_version_hash VARCHAR NOT NULL,
    state JSONB NOT NULL, -- { statement_index, locals, awaiting_task_id }
    created_at TIMESTAMP,
    updated_at TIMESTAMP
)
```

**`workflow_definitions`** - Parsed workflows
```sql
CREATE TABLE workflow_definitions (
    name VARCHAR PRIMARY KEY,
    version_hash VARCHAR NOT NULL,
    source TEXT NOT NULL,
    parsed_steps JSONB NOT NULL, -- Cached AST
    created_at TIMESTAMP
)
```

**`worker_heartbeats`** - Worker liveness
```sql
CREATE TABLE worker_heartbeats (
    worker_id VARCHAR PRIMARY KEY,
    last_heartbeat TIMESTAMP NOT NULL,
    queues VARCHAR[] NOT NULL
)
```

---

## Open Questions

### Scaling Beyond Single Postgres

**Unproven**: How Rhythm behaves with:
- Postgres read replicas
- Horizontal sharding (by queue?)
- Multi-region deployments

**Current assumption**: Single Postgres instance is sufficient for most use cases.

### Lock Contention

**Unknown**: How `SELECT FOR UPDATE SKIP LOCKED` performs under high concurrency (100+ workers).

**Needs testing**: Benchmark with varying worker counts and queue contention.

### Snapshot Evolution

**Unsolved**: How to handle breaking changes to snapshot format while maintaining backward compatibility.

**Options**:
- Version snapshots themselves
- Maintain old interpreters for old versions
- Forced migration tools

---

## Comparison to Replay-Based Execution

| Aspect | Replay (Temporal) | Snapshot (Rhythm) |
|--------|-------------------|-------------------|
| **State representation** | Event history | Direct snapshot |
| **Resume mechanism** | Re-execute from start | Continue from saved position |
| **Determinism required** | Yes | No |
| **Storage growth** | O(events) per workflow | O(1) per workflow |
| **Versioning complexity** | High (code must replay correctly) | Low (hash-based) |
| **Migration ease** | Harder (determinism constraints) | Simpler (explicit snapshots) |
| **Debugging** | Replay history to any point | Only current snapshot |
| **Mental model** | Event sourcing | Checkpointing |

**Neither is strictly better**—they're different trade-offs.

---

## Learn More

- [FAQ](FAQ.md) - Common questions
- [DSL Syntax Reference](WORKFLOW_DSL_FEATURES.md) - Language guide
- [Architecture](.context/ARCHITECTURE.md) - Detailed design docs
