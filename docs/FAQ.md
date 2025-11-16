# Frequently Asked Questions

## General

### What is Rhythm?

Rhythm is a durable workflow execution engine that uses snapshots instead of replay. Write workflows in a DSL, implement tasks in Python or JavaScript (or any language), and run on Postgres with no external orchestrator.

### Is this production-ready?

No. Core execution works, but it's missing critical features:
- Control flow (if/else, loops)
- Observability (metrics, tracing, UI)
- Idempotency keys
- Rate limiting
- Battle-testing

Expect bugs and breaking changes.

### Who is this for?

Developers interested in exploring snapshot-based workflow execution as an alternative to replay-based systems like Temporal. Early adopters willing to experiment with pre-release software.

Not for: Production use cases requiring stability, mature tooling, or proven scale.

---

## How Does This Differ from Temporal?

| | Temporal | Rhythm |
|--------|-----------|---------|
| **Execution model** | Replay event history | Snapshot state |
| **Determinism** | Required (time/random/IO are traps) | Not required |
| **Workflow language** | Python/TypeScript/Go native | DSL (async-style syntax) |
| **Versioning** | Manual migration for in-flight workflows | Self-versioning by hash |
| **Infrastructure** | 6+ services (Frontend, History, Matching, Workers, DB, Visibility) | 2 (Postgres + Workers) |
| **Maturity** | Battle-tested, production-grade | Experimental prototype |

**Not competing**—these are different execution models with different trade-offs. Temporal is production-ready. Rhythm explores whether snapshot-based execution can simplify the mental model.

### Why not just use Temporal?

You should use Temporal if you need:
- Production-ready software
- Mature ecosystem and tooling
- Maximum scale (1000s tasks/sec proven)
- Native language workflows (Python/TypeScript/Go)

Rhythm is an experiment. If determinism constraints and replay complexity bother you, try Rhythm. Otherwise, use Temporal.

---

## Technical Questions

### Why snapshot instead of replay?

**Replay-based execution** (Temporal):
- Store event history (TaskScheduled, TaskCompleted, etc.)
- On resume, replay events from the start
- Workflow code must be deterministic (same inputs → same events)
- Requires careful versioning to maintain compatibility

**Snapshot-based execution** (Rhythm):
- Store execution state directly (locals, stack position)
- On resume, continue from snapshot
- No determinism requirement—code can use time, random, I/O
- Simpler versioning (hash-based)

**Trade-off**: Replay lets you debug by stepping through history. Snapshots are simpler but less debuggable.

### What about exactly-once semantics?

Tasks and snapshots commit in the same Postgres transaction (outbox pattern built-in).

**Example**:
```python
@rhythm.task(queue="orders")
async def process_payment(order_id: str):
    async with db.transaction():
        # Update database
        await db.orders.update(order_id, status="paid")

        # Return result (commits with snapshot)
        return {"success": True}
```

If the worker crashes after executing but before committing, the task rolls back and retries.

**Idempotency keys** (planned) will deduplicate retries at the application level.

### How do you handle snapshot schema changes?

Version workflows by hash. Old workflows keep their schema, new workflows get new schema.

**Example**:
1. Workflow v1 (hash: `abc123`) has 100 in-flight executions
2. You update the workflow (hash changes to `def456`)
3. Old 100 executions keep running on `abc123`
4. New executions use `def456`

**If you need to migrate in-flight workflows**:
- Pause them
- Manually create new executions on new version
- Cancel old executions

**Trade-off**: Less automatic than Temporal, but simpler model.

### Why Postgres-only?

**Simplicity**. One database vs database + Redis + queue system.

**Trade-offs**:
- ✅ Transactional guarantees (tasks + snapshots in same transaction)
- ✅ Simpler operational model
- ✅ No additional infrastructure
- ❌ Lower throughput ceiling than Redis-based queues
- ❌ Unknown scaling beyond single Postgres instance
- ❌ Lock contention under high concurrency (not yet tested)

**Good for**: Most applications (<100 workers, moderate throughput)

**Not for**: Extreme scale (100k+ tasks/sec), multi-region distribution

### What about snapshot size and serialization cost?

**Current implementation**:
- Serialization: JSON via `serde_json`
- Typical size: <1KB per snapshot (statement index + locals)
- Storage: Postgres JSONB column

**No performance testing yet** on:
- Large state (workflows with big objects in locals)
- High-frequency snapshots (tight loops creating many snapshots)

**Planned mitigations**:
- Compression (gzip)
- External blob storage for large state
- Warnings when snapshots exceed threshold

### How does this handle Postgres load?

Workers poll for tasks using `SELECT FOR UPDATE SKIP LOCKED`:

```sql
SELECT * FROM executions
WHERE status = 'pending'
  AND queue = 'orders'
ORDER BY priority DESC
LIMIT 1
FOR UPDATE SKIP LOCKED
```

**`SKIP LOCKED`** prevents workers from blocking each other.

**Polling interval**: Default 1 second (configurable).

**Unknown**:
- Behavior under high concurrency (100+ workers)
- Lock contention characteristics
- Scaling beyond single Postgres instance

**Needs testing**: Benchmark with varying worker counts.

### Can I use this with existing task queues (Celery, RQ, etc.)?

Not directly. Rhythm is a unified system for both tasks and workflows.

However, you can:
- Enqueue Rhythm tasks from Celery workers (via HTTP/RPC)
- Call external APIs from Rhythm tasks
- Gradually migrate from Celery to Rhythm

**Use case**: If you have Celery for simple tasks and want to add workflows, you could:
1. Keep Celery for existing tasks
2. Use Rhythm for new workflows
3. Rhythm workflows call Celery tasks via HTTP

---

## DSL Questions

### Why a DSL instead of Python/TypeScript?

**Problem with native languages**:
- Each language brings determinism challenges
- Requires language-specific linters to catch non-deterministic code
- SDK-specific versioning rules
- Replay testing infrastructure
- Different implementations for each language

**DSL benefits**:
- Runtime doesn't rely on determinism—non-deterministic code is allowed
- Single parser, single execution engine, consistent behavior
- Auto-versioning by content hash
- Write once, call from any language (Python and Node.js teams share workflow definitions)

**DSL costs**:
- Another syntax to learn (though it's minimal—async/await style)
- Limited expressiveness (no control flow yet)
- No language-native tooling (debuggers, IDE autocomplete, type checking)—though this is planned
- Smaller community, fewer examples

**The bet**: For many workflows, the simplicity is worth the constraint.

### Will you add debuggers/IDE support for the DSL?

Yes, planned. The DSL is designed to support:
- LSP (Language Server Protocol) for IDE integration
- Debugger protocol (step through workflows)
- Type checking (static analysis)

Not implemented yet, but architecturally feasible.

### Why JavaScript-like syntax instead of Python-like?

- Familiarity: Most developers know async/await from JS
- Simplicity: Minimal syntax, easy to parse
- JSON integration: Unquoted keys, natural object literals

Could support Python-like syntax in future, but keeping it minimal for now.

### Can I call Python/JS libraries from workflows?

No. Workflows run in the DSL interpreter, not Python/JS runtimes.

**You can**:
- Call tasks (which are Python/JS) from workflows
- Pass data between workflow and tasks
- Implement complex logic in tasks

**Example**:
```javascript
// workflow.flow
workflow(ctx, inputs) {
  // Can't import Python libraries here
  let result = await task("processWithPandas", { data: inputs.csvData })
  // ^ This runs Python code
}
```

```python
# tasks.py
import pandas as pd

@rhythm.task(queue="data")
async def processWithPandas(data: str):
    # Full Python available here
    df = pd.read_csv(data)
    return df.describe().to_dict()
```

---

## Operational Questions

### How do I deploy this?

**Requirements**:
- Postgres 14+
- Python 3.11+ or Node.js 18+ (for workers)

**Deployment steps**:
1. Run migrations: `rhythm migrate`
2. Deploy application code (tasks + workflow .flow files)
3. Start workers: `rhythm worker -q queue1 -q queue2`

**Scaling**:
- Add more workers (horizontal scaling)
- Increase connection pool size
- Partition by queue for traffic isolation

**No separate orchestrator service** to deploy.

### How do I monitor workflows?

Not implemented yet. Planned:
- Web UI for workflow inspection
- OpenTelemetry integration
- Metrics (task throughput, latency, queue depth)
- Tracing (distributed tracing for workflows)

Currently: Query Postgres directly.

```sql
-- See pending workflows
SELECT * FROM executions WHERE status = 'pending'

-- See in-progress workflows
SELECT * FROM executions WHERE status = 'running'

-- Inspect workflow state
SELECT state FROM workflow_execution_context WHERE execution_id = '...'
```

### How do I handle secrets?

Standard practices:
- Environment variables
- Secret management systems (Vault, AWS Secrets Manager, etc.)
- Don't hardcode in workflow .flow files

**Example**:
```python
# tasks.py
import os

@rhythm.task(queue="payments")
async def charge_card(amount: float):
    api_key = os.environ["STRIPE_API_KEY"]
    # Use api_key...
```

Workflows pass data, tasks access secrets.

### Can I run this on multiple Postgres instances?

Not tested. Current design assumes single Postgres instance.

**Possible approaches** (unproven):
- Read replicas for query load
- Sharding by queue (different queues on different Postgres instances)
- Multi-region with async replication (eventual consistency)

**Unknown**: How worker coordination works across instances.

### What happens if Postgres goes down?

Workers can't claim tasks or update snapshots. System halts until Postgres recovers.

**Mitigation**:
- Postgres HA (streaming replication, Patroni, etc.)
- Connection retry logic in workers
- Monitoring and alerting

No special Rhythm-specific handling—rely on Postgres HA.

---

## Comparison Questions

### How does this compare to Airflow?

**Airflow**: DAG scheduler for batch data pipelines.
- Define dependencies as DAGs
- Scheduled runs (cron-like)
- Not designed for long-running, event-driven workflows

**Rhythm**: Durable workflow execution for async processes.
- Define workflows as code (DSL)
- Event-driven (start workflows on demand)
- Designed for multi-step, long-running processes

**Use Airflow for**: ETL, batch data processing, scheduled jobs
**Use Rhythm for**: Order processing, user onboarding, saga patterns

### How does this compare to Prefect?

Similar to Airflow comparison. Prefect is a modern data workflow orchestrator, Rhythm is a durable execution engine.

**Key difference**: Prefect focuses on data pipelines, Rhythm focuses on business process orchestration.

### How does this compare to Celery?

**Celery**: Distributed task queue.
- Simple task execution
- No durable workflows
- Requires separate broker (Redis/RabbitMQ)

**Rhythm**: Task queue + durable workflows.
- Unified interface (tasks and workflows use same model)
- Postgres-only, no separate broker

**Migration path**: Use Rhythm for both tasks and workflows instead of Celery + homegrown orchestration.

### How does this compare to AWS Step Functions?

**Step Functions**: AWS-managed workflow orchestrator.
- Define workflows in ASL (Amazon States Language)
- Fully managed, serverless
- AWS-specific

**Rhythm**: Self-hosted workflow orchestrator.
- Define workflows in custom DSL
- Deploy anywhere (Postgres available)
- Not tied to cloud provider

**Use Step Functions if**: You're all-in on AWS, want fully managed
**Use Rhythm if**: You want self-hosted, cloud-agnostic

---

## Contributing

### Can I contribute?

Not seeking contributions yet (pre-release, rapid changes).

Once stable, contributions welcome for:
- Language adapters (Go, Ruby, Rust, etc.)
- Tooling (LSP, debugger, UI)
- Documentation and examples

### How can I help?

**Feedback** is most valuable right now:
- Try it out, report bugs
- Share whether the snapshot model makes sense
- Suggest missing features or design improvements

[Open an issue](https://github.com/yourusername/rhythm/issues) or [discussion](https://github.com/yourusername/rhythm/discussions).

---

## Roadmap

### What's next?

**Priority 1**: Control flow (if/else, loops, expressions)—DSL unusable without this
**Priority 2**: Observability (what's running, where it's stuck)
**Priority 3**: Production features (idempotency, rate limiting, retention)

[Full roadmap](.context/TODO.md)

### When will this be production-ready?

No timeline yet. Depends on:
- Completing control flow implementation
- Adding critical production features
- Battle-testing with real workloads
- Community feedback and iteration

**Months, not weeks.**
