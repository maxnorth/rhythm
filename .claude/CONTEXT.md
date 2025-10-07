# Currant - Project Context

> **Instructions for Claude**: Read this file at the start of each session (via `/context` command). After making significant architectural decisions or design changes, UPDATE this file to preserve context for future conversations.

## What is Currant?

Currant is a **lightweight durable execution framework** that enables building reliable, multi-step workflows using only PostgreSQL - no external orchestrator needed.

**Competitors**: Temporal, DBOS Transact, AWS Step Functions
**Key Differentiator**: Postgres-only architecture (no separate orchestrator/server required)

## Architecture Overview

### Core Design: Rust + Language Adapters

```
┌─────────────────────────────────┐
│     Language Adapters           │
│  (Python ✅, Node.js 🚧)        │
│  - Decorators (@job, @workflow) │
│  - Worker loops                 │
│  - Workflow replay logic        │
└────────────┬────────────────────┘
             │ FFI (PyO3/Neon)
┌────────────▼────────────────────┐
│       Rust Core Engine          │
│  - Database operations (sqlx)   │
│  - Execution management         │
│  - Worker coordination          │
│  - LISTEN/NOTIFY                │
└────────────┬────────────────────┘
             │
┌────────────▼────────────────────┐
│       PostgreSQL Only           │
│  - executions table             │
│  - worker_heartbeats            │
│  - workflow_signals             │
│  - pg_notify channels           │
└─────────────────────────────────┘
```

### Key Components

**Rust Core** (`/core`):
- `lib.rs` - PyO3/FFI bindings exposing Rust functions to language adapters
- `db.rs` - Database connection pooling (sqlx)
- `executions.rs` - CRUD operations for jobs/workflows/activities
- `worker.rs` - Worker heartbeat, dead worker detection, failover
- `signals.rs` - Workflow signal management
- `types.rs` - Shared data structures

**Python Adapter** (`/python/currant`):
- `decorators.py` - `@job`, `@activity`, `@workflow` decorators
- `registry.py` - Function registry for decorated functions
- `worker.py` - Worker loop (claim → execute → report)
- `client.py` - `.queue()`, `send_signal()` client API
- `context.py` - Workflow context (`wait_for_signal()`, `get_version()`)
- `rust_bridge.py` - FFI wrapper with JSON serialization

**Node.js Adapter** (`/node`) - 🚧 In Progress
- Native Rust bindings via Neon
- Similar API to Python adapter

## Core Design Decisions

### 1. Why Rust Core + Language Adapters?

**Decision**: Implement core logic in Rust once, expose via FFI to language-specific adapters.

**Rationale**:
- **Performance**: Rust handles all database operations, critical for low-latency task claiming
- **Correctness**: Type safety and ownership prevent entire classes of concurrency bugs
- **Polyglot**: Core logic written once, N language adapters are thin wrappers
- **Testability**: Core engine thoroughly tested in Rust

**Alternatives Rejected**:
- Pure language implementations → code duplication, inconsistent behavior
- HTTP/gRPC service → network overhead, deployment complexity

### 2. Why Postgres-Only (No External Orchestrator)?

**Decision**: Achieve worker coordination entirely through Postgres primitives.

**How**:
- Worker heartbeats stored in `worker_heartbeats` table (updated every 5s)
- Dead worker detection via timestamp checks (30s timeout)
- Work recovery via `UPDATE ... WHERE worker_id = $dead_worker`
- LISTEN/NOTIFY for instant task pickup
- Row-level locking (`SELECT ... FOR UPDATE SKIP LOCKED`) for claiming

**Comparison**:
- **DBOS Transact**: Requires separate Conductor service for coordination
- **Temporal**: Requires dedicated server cluster
- **Currant**: Only Postgres

**Trade-offs**:
- ✅ Simpler deployment (one fewer service)
- ✅ Lower operational overhead
- ⚠️ Postgres becomes single point of failure (mitigated by HA Postgres)

### 3. Why PyO3 Instead of HTTP/gRPC?

**Decision**: Use direct FFI bindings (PyO3 for Python, Neon for Node.js).

**Rationale**:
- **Performance**: Function calls ~100x faster than network round-trips
- **Simplicity**: No separate server process, simpler deployment
- **Type Safety**: PyO3/Neon provide type-safe FFI boundaries
- **Developer Experience**: Import and use like a native library

**When this might change**: If we need to support languages without good Rust FFI (e.g., PHP, Ruby), we'd add an HTTP API as a fallback.

### 4. Workflow Replay (Temporal-Style)

**Decision**: Use deterministic replay for workflow suspension/resumption.

**How it works**:
1. Workflow calls `activity.run()` → raises `WorkflowSuspendException`
2. Worker catches exception, creates activity execution, suspends workflow
3. Activity completes (separate execution)
4. Workflow resumes, re-executes from beginning
5. Previous activities return cached results from checkpoint
6. Workflow continues to next activity or completes

**Why this approach**:
- ✅ Transparent to developers (just write normal async code)
- ✅ No need for custom DSL or workflow graph definitions
- ✅ Handles arbitrary control flow (if/else, loops)
- ⚠️ Requires deterministic workflow code (no random(), time.now() in workflow logic)

## Execution Model

### Three Execution Types

1. **Job**: Simple async task (like Celery)
   - Single unit of work
   - Retries on failure
   - Example: Send email, process payment

2. **Activity**: Workflow step
   - Always runs within a workflow context
   - Suspends parent workflow until complete
   - Example: Charge card, send receipt

3. **Workflow**: Multi-step orchestration
   - Coordinates multiple activities
   - Survives crashes via checkpointing
   - Example: Order processing, approval flow

### Queue-First Design

**Everything is queued by default**. No synchronous execution model.

```python
# Enqueue (returns immediately with execution_id)
job_id = await send_email.queue(to="user@example.com", subject="Hi")

# Worker picks it up asynchronously
```

**Why**: Decouples producers from workers, enables scaling, built-in reliability.

## Current State (as of 2025-10-06)

### ✅ Implemented
- Rust core with full execution management
- Python adapter (complete, tested, all scripts verified)
- Node.js adapter (complete, native bindings working, 23 tests passing)
- Worker coordination and failover
- Workflow signals
- Versioning support
- CLI tools
- Migrations

### 📋 Future Roadmap
- Go adapter
- Distributed tracing integration
- Metrics and observability
- Workflow visualization dashboard
- Workflow testing utilities

## Build Instructions

### Python (PyO3)
```bash
cd core
maturin develop --features python
```

### Node.js (NAPI-RS)
```bash
cd node
npm run build:native  # Builds core without python feature
npm run build         # Builds TypeScript
npm test             # 23 tests
```

### Key Points
- Rust core supports both Python and Node.js via feature flags
- PyO3 and NAPI don't conflict (feature flags prevent this)
- Native bindings consolidated into main packages (not separate packages)
- Database: PostgreSQL in Docker at `postgresql://workflows:workflows@localhost/workflows`

## Development Guidelines

### When Adding Features

1. **Core logic goes in Rust** if it involves:
   - Database operations
   - Worker coordination
   - Execution state management
   - Performance-critical paths

2. **Adapter logic** handles:
   - Language-specific APIs
   - Function registry
   - Serialization/deserialization
   - Workflow replay mechanics

3. **Always update this file** when making architectural decisions that future conversations need to know about.

### Testing Philosophy

- Rust core: Unit tests + integration tests with test database
- Adapters: Function registry, worker loop, replay logic
- E2E: Full worker + enqueue scenarios

### Database Schema

Critical tables:
- `executions`: All jobs/activities/workflows (polymorphic)
- `worker_heartbeats`: Worker liveness tracking
- `workflow_signals`: External events for workflows

See `core/migrations/` for schema details.

## Common Patterns

### Adding a New Language Adapter

1. Create FFI bindings to Rust core (e.g., Neon for Node.js)
2. Implement decorators/annotations for function registration
3. Implement worker loop: claim → execute → report
4. Implement workflow replay logic (handle suspend/resume)
5. Implement client API (`.queue()`, `send_signal()`)

### Debugging Workflow Issues

- Check `executions` table for state
- Look at `checkpoint` JSON for replay history
- Worker logs show which activities executed
- Use `get_workflow_activities()` to see child executions

---

**Remember**: Update this file when making significant architectural decisions!
