# Additional Feature Requirements - Research Findings

> **Generated**: 2025-10-12
> **Sources**: Temporal, DBOS, Celery, Sidekiq, BullMQ, AWS Step Functions, Azure Logic Apps

This document captures features and patterns from mature workflow/task queue systems that Currant should consider implementing.

---

## Priority 1: Core Execution Guarantees & Semantics (CRITICAL)

### 1. **Idempotency Guarantees**
**What**: Ensure operations produce the same result regardless of how many times they execute
**Why**: Foundation for reliable retry logic and exactly-once semantics
**Implementation needs**:
- Request ID tracking for tasks/workflows
- Deduplication window (e.g., 24 hours)
- Idempotency keys in execution metadata
- Database constraints to prevent duplicate execution

**References**:
- Temporal guarantees "at least once" task execution but recommends idempotent Tasks
- DBOS transactions commit exactly once, steps retry but never re-execute after completion
- Kafka uses idempotent producer to prevent duplicates

### 2. **Execution Semantics Clarity**
**What**: Document exactly what guarantees Currant provides
**Why**: Users need to know when to implement idempotency vs rely on framework
**Guarantees to define**:
- Tasks: At-least-once? Exactly-once?
- Workflows: Deterministic replay guarantees
- Tasks: Retry behavior and completion guarantees
- Database transactions: ACID guarantees during execution

### 3. **Dead Letter Queue (DLQ)**
**What**: Queue for tasks that fail after max retry attempts
**Why**: Prevents endless retry loops, provides debugging capability
**Implementation needs**:
- Automatic DLQ routing after N retries (configurable per queue/task)
- DLQ inspection via CLI: `currant dlq list`, `currant dlq inspect <id>`
- DLQ reprocessing: `currant dlq retry <id>` (fix code, then retry)
- Failure metadata: error message, stack trace, retry count
- DLQ retention policy separate from main queues

**References**: Used by AWS SQS, Azure Service Bus, Kafka, RabbitMQ, BullMQ

### 4. **Retry Policies & Exponential Backoff**
**What**: Configurable retry behavior with intelligent backoff
**Why**: Handle transient failures without overwhelming systems
**Implementation needs**:
- Per-task retry configuration (max attempts, backoff strategy)
- Per-queue retry defaults
- Backoff strategies: exponential, linear, fixed
- Jitter to prevent thundering herd
- Retry predicate (which errors are retryable)

**Example**:
```python
@currant.task(
    queue="api-calls",
    retry_policy=RetryPolicy(
        max_attempts=5,
        backoff="exponential",
        initial_interval="1s",
        max_interval="60s",
        backoff_coefficient=2.0,
        jitter=0.1
    )
)
```

---

## Priority 2: Queue Management & Control (HIGH PRIORITY)

### 5. **Rate Limiting & Concurrency Control**
**What**: Control throughput and parallelism at queue and task level
**Why**: Prevent overwhelming downstream APIs, respect rate limits, resource management
**Implementation needs**:
- **Queue-level rate limiting**: max tasks/sec (e.g., 100 tasks/sec for API queue)
- **Queue-level concurrency**: max concurrent executions (e.g., 10 parallel for DB-heavy queue)
- **Per-task concurrency**: max parallel executions of specific task type
- **Token bucket algorithm** for smooth rate limiting
- **Leaky bucket algorithm** for burst handling

**Configuration examples**:
```toml
[queues.api-calls]
rate_limit = "100/sec"        # 100 tasks per second max
max_concurrent = 10            # Max 10 parallel executions

[queues.emails]
rate_limit = "1000/min"       # 1000 emails per minute
max_concurrent = 50
```

**References**:
- GCP Cloud Tasks: max dispatches/sec + max concurrent dispatches
- BullMQ: rate limiter vs delay options
- Sidekiq: concurrency limits per queue

### 6. **Queue Prioritization**
**What**: Execute important tasks before less important ones
**Why**: Business-critical tasks need faster processing
**Two levels needed**:

**A. Queue-level priority** (which queue to poll first):
- Workers poll queues in priority order or weighted round-robin
- Example: `--queues high:3,default:2,low:1` (3x more high-priority polls)

**B. Task-level priority** (within a single queue):
- Add `priority` column to executions table (integer, higher = more urgent)
- Modify claim query: `ORDER BY priority DESC, scheduled_at ASC`
- Ad-hoc priority boost: `currant reprioritize <id> --priority 10`

**References**: Supported by Celery, Sidekiq, BullMQ

### 7. **Queue Lifecycle Management**
**What**: Pause, drain, purge, move operations on queues
**Why**: Operational control during deployments, incidents, migrations
**Commands needed**:
```bash
currant queue list                      # Show all queues with stats
currant queue inspect <name>            # Detailed queue info
currant queue pause <name>              # Stop claiming new tasks
currant queue resume <name>             # Resume claiming
currant queue drain <name>              # Wait for in-flight to complete
currant queue purge <name>              # Delete all pending tasks
currant queue move <id> --to <queue>    # Move task to different queue
currant queue stats <name>              # Metrics: depth, throughput, latency
```

### 8. **Queue Metrics & Visibility**
**What**: Real-time statistics about queue health
**Metrics needed**:
- Queue depth (waiting tasks)
- Processing rate (tasks/sec completed)
- Average wait time (time in queue before execution)
- Average execution time
- Success/failure rates
- Worker count per queue
- Oldest task age

**Export to**: Prometheus, OTLP, CLI, Management UI

---

## Priority 3: Workflow Patterns & Advanced Features (HIGH PRIORITY)

### 9. **Saga Pattern & Compensation**
**What**: Long-running transactions with rollback capability
**Why**: Distributed transactions across services need coordinated rollback on failure
**Implementation needs**:
- Compensation handlers (inverse operations)
- Automatic compensation triggering on failure
- Saga execution tracking
- Partial rollback capability

**Example pattern**:
```python
@currant.workflow
async def book_trip(ctx, user_id, trip_details):
    # Forward transactions
    flight_id = await ctx.execute_activity(book_flight, trip_details.flight)
    hotel_id = await ctx.execute_activity(book_hotel, trip_details.hotel)
    car_id = await ctx.execute_activity(book_car, trip_details.car)

    try:
        payment = await ctx.execute_activity(charge_payment, ...)
    except PaymentFailure:
        # Compensation (reverse order)
        await ctx.execute_activity(cancel_car, car_id)
        await ctx.execute_activity(cancel_hotel, hotel_id)
        await ctx.execute_activity(cancel_flight, flight_id)
        raise
```

**References**:
- Temporal saga pattern documentation
- Azure Logic Apps compensation
- AWS Step Functions saga orchestration

### 10. **Child Workflows**
**What**: Workflows can spawn other workflows
**Why**: Partition large workloads, separate concerns, different worker pools
**Implementation needs**:
- Parent-child relationship tracking
- Parent close policies: `terminate`, `cancel`, `abandon` (keep running)
- Child workflow awaiting strategies:
  - Synchronous (block until child completes)
  - Asynchronous (fire-and-forget)
  - Parallel (spawn multiple, wait for all)
- Separate event histories (scalability)
- Trace context propagation (parent trace_id → child)

**Use case**: Parent workflow spawns 1,000 child workflows, each processing a batch of 1,000 items = 1M items processed with bounded history size per workflow

**References**: Temporal, Cadence, Google Cloud Workflows, AWS Step Functions

### 11. **Continue-As-New**
**What**: Start a new workflow execution with fresh history but continue from current state
**Why**: Workflows can run indefinitely without hitting history size limits
**Implementation needs**:
- History size limits: warn at 10K events, terminate at 50K events or 50MB
- `continue_as_new()` API to restart with new history
- State transfer between old and new execution
- Same workflow ID, new run ID
- Automatic or manual triggering

**Use case**:
- Infinite loops (process queue forever)
- Cron workflows that run for years
- Long-running subscriptions

**References**:
- Temporal: 51,200 event or 50MB limit
- DBOS: No explicit limit mentioned (PostgreSQL-backed)
- AWS Step Functions: 25,000 events or 1 year limit

### 12. **Task Batching & Parallel Execution**
**What**: Group similar tasks for efficient parallel processing
**Why**: Reduce overhead, maximize throughput, optimize resource usage
**Implementation needs**:
- Batch size configuration
- Parallel task execution within workflows
- Dynamic parallelism (fan-out/fan-in patterns)
- Batch timeout (don't wait forever for full batch)

**Example**:
```python
@currant.workflow
async def process_orders(ctx, order_ids):
    # Fan-out: execute 100 Tasks in parallel
    futures = [
        ctx.execute_activity(process_order, order_id)
        for order_id in order_ids
    ]

    # Fan-in: wait for all to complete
    results = await ctx.gather(*futures)
    return results
```

**References**: Spring Batch, Azure Batch, Temporal parallel Tasks

---

## Priority 4: Scheduling & Time-Based Features (CORE FEATURE)

### 13. **Cron Scheduling**
**What**: Recurring tasks based on cron patterns
**Why**: Core feature for periodic tasks (reports, cleanups, syncs)
**Implementation needs**:
- Cron expression parsing (unix cron with optional seconds)
- Timezone support (UTC default, configurable per cron)
- Catch-up behavior: run missed executions on restart?
- Overlap handling: allow/prevent concurrent cron executions
- Cron registration API

**Example**:
```python
# Decorator-based
@currant.cron("0 3 * * *", timezone="America/New_York")
async def daily_report():
    pass

# Programmatic
currant.schedule_cron(
    name="daily-report",
    schedule="0 3 * * *",
    function=daily_report,
    timezone="America/New_York",
    catch_up=False
)
```

**References**: Celery Beat, Sidekiq-cron, BullMQ repeatable tasks

### 14. **Delayed Execution & Scheduling**
**What**: Execute task at specific time or after delay
**Why**: Fundamental scheduling primitive
**Implementation needs**:
- `schedule_at(datetime)`: execute at specific time
- `schedule_in(duration)`: execute after delay
- Durable sleep (survives restarts)
- Database-backed delayed set (not memory)
- Efficient polling of due tasks

**Example**:
```python
# Schedule for specific time
await currant.schedule_at(
    send_reminder,
    scheduled_time=meeting_time - timedelta(hours=1),
    args=[user_id, meeting_id]
)

# Schedule after delay
await currant.schedule_in(
    retry_failed_payment,
    delay=timedelta(minutes=30),
    args=[payment_id]
)
```

**References**: BullMQ delayed tasks, Sidekiq scheduled tasks

### 15. **Durable Timers & Sleep**
**What**: Workflows can sleep with state preserved across restarts
**Why**: Wait periods between Tasks, timeouts, polling intervals
**Implementation needs**:
- `await ctx.sleep(duration)` in workflows
- Timer stored in database (wakeup time persisted)
- Timer claims by workers (distributed timer processing)
- Timer cancellation on workflow cancellation

**References**:
- Temporal durable timers
- DBOS durable sleep (saves wakeup time in database)

---

## Priority 5: Operational & Production Readiness (IMPORTANT)

### 16. **Workflow Versioning**
**What**: Deploy new workflow code without breaking in-flight executions
**Why**: Long-running workflows may span multiple deployments
**Implementation needs**:
- Version field on workflow definitions
- Backward compatibility checks
- Version routing (old executions use old code)
- Multi-version worker support (single worker can run v1 and v2)
- Incompatible change detection

**Compatible changes** (safe):
- Adding new Tasks
- Changing task implementation (if idempotent)
- Adding optional parameters

**Incompatible changes** (breaking):
- Changing workflow logic order
- Removing Tasks
- Changing task signatures
- Changing branch conditions (determinism violation)

**References**:
- Temporal versioning APIs
- Cadence versioning
- Microsoft Entra workflow versioning

### 17. **Search Attributes & Execution Metadata**
**What**: Custom indexed fields for filtering/searching executions
**Why**: Find specific workflows, operational queries, debugging
**Implementation needs**:
- Custom metadata on executions (key-value pairs)
- Indexed search attributes (queryable fields)
- Search API: `currant search "status=failed AND queue=api-calls"`
- Support for: string, number, datetime, boolean attributes
- List filtering in UI/CLI

**Example**:
```python
await currant.start_workflow(
    process_order,
    workflow_id=f"order-{order_id}",
    search_attributes={
        "customer_id": customer_id,
        "order_total": order_total,
        "region": "us-west",
        "priority": "high"
    }
)

# Later: search for specific executions
results = currant.search_workflows(
    "customer_id='CUST123' AND order_total > 1000"
)
```

**References**: Temporal search attributes, Azure Logic Apps filtering

### 18. **Monitoring & Observability UI**
**What**: Web-based monitoring dashboard
**Why**: Visibility into system health, debugging, operations
**Features needed**:
- Real-time worker status
- Queue depths and throughput
- Execution search and filtering
- Workflow DAG visualization
- Execution timeline view
- Error logs and stack traces
- Performance metrics dashboards
- Alerting configuration

**Inspiration**:
- Flower (Celery monitoring)
- Temporal Web UI
- Sidekiq Web Dashboard

**Integration**: Prometheus + Grafana for metrics, custom UI for execution details

---

## Priority 6: Resilience & Error Handling (IMPORTANT)

### 19. **Circuit Breaker Pattern**
**What**: Automatically pause queue on high failure rate
**Why**: Prevent cascading failures, give downstream services time to recover
**Implementation needs**:
- Failure rate threshold (e.g., pause if >50% fail in 5 min)
- Circuit states: closed (normal), open (paused), half-open (testing)
- Auto-recovery: try single task after cooldown, resume if succeeds
- Manual override: force open/close circuit

**Configuration**:
```toml
[queues.external-api]
circuit_breaker.enabled = true
circuit_breaker.failure_threshold = 0.5    # 50% failure rate
circuit_breaker.window = "5m"              # Over 5 minute window
circuit_breaker.cooldown = "1m"            # Wait 1 min before retry
circuit_breaker.min_requests = 10          # Need 10+ requests to trigger
```

### 20. **Graceful Degradation & Fallbacks**
**What**: Execute fallback logic when primary path fails
**Why**: Continue operation with reduced functionality instead of total failure
**Implementation needs**:
- Fallback Tasks in workflows
- Timeout-based fallback triggering
- Error-based fallback triggering

**Example**:
```python
@currant.workflow
async def get_user_profile(ctx, user_id):
    try:
        # Try primary data source (3 sec timeout)
        return await ctx.execute_activity(
            fetch_from_primary_db,
            user_id,
            timeout=timedelta(seconds=3)
        )
    except TimeoutError:
        # Fallback to cache
        return await ctx.execute_activity(fetch_from_cache, user_id)
```

### 21. **Singleton Tasks (Leader Election)**
**What**: Ensure only one instance of a task runs across cluster
**Why**: Maintenance tasks, cron tasks, cleanup operations
**Implementation needs**:
- Distributed lock acquisition before execution
- Database-based locking (PostgreSQL advisory locks)
- Lock timeout/renewal (heartbeat)
- Lock release on completion or failure
- CLI command: `currant run-once <job_name>`

**Use cases**:
- Database cleanup/vacuum
- Report generation (prevent duplicates)
- External system polling
- Leader-elected maintenance tasks

**Example**:
```python
@currant.task(singleton=True, singleton_timeout="10m")
async def cleanup_old_executions():
    # Only one worker in cluster will execute this
    pass
```

**References**:
- Redis SETNX for distributed locking
- PostgreSQL advisory locks
- Kubernetes leader election

---

## Priority 7: Developer Experience & Testing (IMPORTANT)

### 22. **Workflow Testing Utilities**
**What**: Tools to test workflows in isolation
**Why**: Enable unit testing without full infrastructure
**Features needed**:
- Mock task implementation
- Time control (fast-forward timers)
- Deterministic replay testing
- Workflow history inspection
- Assertion helpers

**Example**:
```python
def test_order_workflow():
    env = WorkflowTestEnvironment()

    # Mock Tasks
    env.mock_activity(charge_payment, return_value={"tx_id": "123"})
    env.mock_activity(send_confirmation_email)

    # Run workflow
    result = env.run_workflow(process_order, order_id="ORDER123")

    # Assertions
    assert result.status == "completed"
    env.assert_activity_called(charge_payment, times=1)
    env.assert_activity_called(send_confirmation_email, times=1)
```

**References**: Temporal testing framework, pytest fixtures

### 23. **Local Development Mode**
**What**: Simplified setup for local development
**Why**: Lower barrier to entry, fast iteration
**Features needed**:
- In-memory mode (no PostgreSQL required)
- Auto-reload on code changes
- Detailed debug logging
- UI with hot reload
- Docker Compose quick-start

**Example**:
```bash
# Start in local mode (in-memory)
currant dev

# With hot reload
currant dev --watch

# With UI
currant dev --ui
```

### 24. **CLI Autocomplete & Help**
**What**: Shell completion for CLI commands
**Why**: Better UX, discoverability
**Implementation**: Generate completion scripts for bash/zsh/fish

---

## Priority 8: Security & Multi-Tenancy (FUTURE)

### 25. **Authentication & Authorization**
**What**: Control who can execute tasks, view executions, manage queues
**Why**: Production deployments need access control
**Features needed**:
- API authentication (API keys, JWT)
- Role-based access control (RBAC)
- Per-queue permissions
- Audit logging
- Multi-tenancy support (isolated workspaces)

### 26. **Secrets Management**
**What**: Secure storage and injection of secrets into workflows
**Why**: API keys, passwords, tokens need secure handling
**Integration with**:
- Environment variables
- HashiCorp Vault
- AWS Secrets Manager
- Azure Key Vault
- Kubernetes secrets

### 27. **Network Policies & Isolation**
**What**: Control network access between components
**Why**: Security best practices, compliance
**Features**:
- Worker isolation (different trust zones)
- Queue-based routing (route tasks to specific worker pools)
- Namespace isolation

---

## Priority 9: Performance & Scalability (ONGOING)

### 28. **Horizontal Scalability**
**What**: Linear performance scaling with more workers
**Why**: Handle growing workloads
**Considerations**:
- Connection pooling (PostgreSQL connection limits)
- Claim query optimization (index usage)
- Worker heartbeat efficiency
- Distributed caching (reduce DB load)

### 29. **High Availability & Failover**
**What**: System continues operating despite failures
**Why**: Production reliability
**Features**:
- Worker failure detection and task reassignment
- Database failover (PostgreSQL HA setup)
- Zero-downtime deployments
- Graceful shutdown with task draining

### 30. **Performance Benchmarking & Regression Testing**
**What**: Continuous performance validation
**Why**: Detect performance regressions early
**Benchmarks needed**:
- Throughput: tasks/sec at various scales
- Latency: p50, p95, p99 claim times
- Concurrency: performance under high parallelism
- Long-running: workflows with 10K+ Tasks
- Large tables: performance with millions of executions
- Cluster mode: multi-worker coordination overhead

**References**: Existing `bench` command is good start, expand it

---

## Priority 10: Additional Patterns & Niceties (NICE-TO-HAVE)

### 31. **Human-in-the-Loop**
**What**: Workflows that pause for human approval/input
**Why**: Approval workflows, forms, manual verification
**Implementation**:
- Signal-based (already planned)
- Webhook callbacks
- Form generation
- Timeout handling (if no response in X time)

### 32. **Workflow Pause/Resume**
**What**: Manually pause workflow execution
**Why**: Debugging, controlled execution, maintenance
**Commands**: `currant workflow pause <id>`, `currant workflow resume <id>`

### 33. **Dynamic Configuration Updates**
**What**: Update queue config without restart
**Why**: Operational flexibility
**Example**: Change rate limit on-the-fly during incident

### 34. **Execution History Archival**
**What**: Move old execution data to cheaper storage
**Why**: Keep database size manageable, reduce costs
**Implementation**:
- Export to S3/blob storage
- Compressed format
- Searchable archive
- Restore capability

### 35. **Multi-Region Support**
**What**: Deploy workers in different regions/datacenters
**Why**: Latency, compliance, availability
**Considerations**:
- Database replication
- Cross-region execution
- Data sovereignty

---

## Summary Table: Feature Priority Matrix

| Priority | Feature Category | Blocking? | Effort | Impact |
|----------|-----------------|-----------|--------|--------|
| P1 | Execution Guarantees (Idempotency, DLQ, Retry) | Yes | Medium | Critical |
| P2 | Queue Management (Rate Limiting, Priority, Lifecycle) | Yes | High | Critical |
| P3 | Workflow Patterns (Saga, Child Workflows, Continue-As-New) | No | High | High |
| P4 | Scheduling (Cron, Delays, Timers) | Yes | Medium | Critical |
| P5 | Operational (Versioning, Search, Monitoring UI) | No | High | High |
| P6 | Resilience (Circuit Breaker, Singleton Tasks) | No | Medium | Medium |
| P7 | Developer Experience (Testing, Local Dev) | No | Medium | High |
| P8 | Security (Auth, Secrets, Multi-Tenancy) | No | High | Medium |
| P9 | Performance (Scalability, HA, Benchmarking) | No | Ongoing | High |
| P10 | Additional Patterns (Human-in-Loop, Pause/Resume) | No | Low | Low |

---

## Comparison: What Currant Has vs Needs

### ✅ Already Have
- Workflow replay (deterministic execution)
- Worker coordination (claim/heartbeat)
- Basic queue support
- Task execution
- Signal handling
- Python adapter (mature)
- Database-backed durability

### 🚧 Partially Have (Need Enhancement)
- Benchmarking (exists, but needs expansion)
- CLI (exists, needs restructuring per TODO.md)
- Observability (designed in TRACING_DESIGN.md, not implemented)

### ❌ Missing (Priority 1-2)
- Idempotency guarantees
- Dead letter queue
- Retry policies (exponential backoff)
- Rate limiting
- Concurrency control
- Queue prioritization
- Queue lifecycle management
- Cron scheduling
- Delayed execution
- Durable timers

### ❌ Missing (Priority 3-5)
- Saga pattern / compensation
- Child workflows
- Continue-as-new
- Task batching / parallel execution
- Workflow versioning
- Search attributes
- Monitoring UI

---

## Next Steps

1. **Review this document** and prioritize features based on Currant's goals
2. **Update TODO.md** with selected features from this research
3. **Create design documents** for Priority 1-2 features (like TRACING_DESIGN.md)
4. **Break down into implementable tasks** with clear acceptance criteria
5. **Begin implementation** starting with execution guarantees and queue management

