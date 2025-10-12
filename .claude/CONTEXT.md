# Currant - Project Context

> **Instructions for Claude**: Read this file at the start of each session (via `/context` command). After making significant architectural decisions or design changes, UPDATE this file to preserve context for future conversations.

## What is Currant?

Currant is a **lightweight durable execution framework** that enables building reliable, multi-step workflows using only PostgreSQL - no external orchestrator needed.

**Language Support**: Designed to support any language with FFI capabilities. Initial development focuses on Python and Node.js adapters.

**Competitors**: Temporal, DBOS Transact, AWS Step Functions
**Key Differentiator**: Postgres-only architecture (no separate orchestrator/server required)

## Architecture Overview

### Core Design: Rust + Language Adapters

```
┌─────────────────────────────────┐
│     Language Adapters           │
│  (Any language with FFI)        │
│  Python ✅  Node.js ✅          │
│  Future: Go, Rust, etc.         │
│  - Decorators (@job, @workflow) │
│  - Worker loops                 │
│  - Workflow replay logic        │
└────────────┬────────────────────┘
             │ FFI (PyO3, NAPI-RS, CGO, etc.)
┌────────────▼────────────────────┐
│       Rust Core Engine          │
│  - Database operations (sqlx)   │
│  - Execution management         │
│  - Worker coordination          │
│  - Language-agnostic interface  │
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

**Node.js Adapter** (`/node`):
- Native Rust bindings via NAPI-RS
- Similar API to Python adapter
- 23 tests passing

**Future Adapters**:
- Any language with FFI support (Go, Rust native, Ruby, etc.)
- Core's language-agnostic design enables easy integration

## Core Design Decisions

### 1. Why Rust Core + Language Adapters?

**Decision**: Implement core logic in Rust once, expose via FFI to language-specific adapters. Support any language with FFI capabilities.

**Rationale**:
- **Performance**: Rust handles all database operations, critical for low-latency task claiming
- **Correctness**: Type safety and ownership prevent entire classes of concurrency bugs
- **Polyglot**: Core logic written once, any language can create thin FFI wrappers
- **Testability**: Core engine thoroughly tested in Rust
- **Universal**: Works with any language that can call C FFI (Python/PyO3, Node/NAPI, Go/CGO, etc.)

**Initial Focus**: Python and Node.js adapters during development phase

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
- Additional language adapters (Go, Rust native, Ruby, etc.)
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
- Rust core is language-agnostic with separate binding crates per language
- Currently supports Python (PyO3) and Node.js (NAPI-RS)
- Can support any language with FFI capabilities
- Native bindings live in `<lang>/native/` directories
- Database: PostgreSQL in Docker at `postgresql://workflows:workflows@localhost/workflows`

## Development Guidelines

### When Adding Features

1. **Core logic goes in Rust (`/core`)** if it involves:
   - Database operations
   - Worker coordination
   - Execution state management
   - Performance-critical paths
   - **CLI commands and argument parsing** (unified across all languages)

2. **Adapter logic** handles:
   - Language-specific APIs
   - Function registry
   - Serialization/deserialization
   - Workflow replay mechanics
   - **FFI bindings in `<lang>/native/` directories** (PyO3, NAPI-RS, etc.)

3. **Always update this file** when making architectural decisions that future conversations need to know about.

### Critical Architectural Principle: Language Bindings Separation

**IMPORTANT**: `core/` is a pure Rust library with NO language-specific bindings. It's designed to be universal - any language with FFI capabilities can integrate with it.

Language bindings live in separate crates:
- **Python**: `python/native/` - PyO3 bindings that import `currant-core`
- **Node.js**: `node/native/` - NAPI-RS bindings that import `currant-core`
- **Future languages**: Follow the same pattern (e.g., `go/native/` with CGO, `ruby/native/` with Rutie, etc.)

**Why this matters**:
- ✅ Core stays clean and language-agnostic - supports any language
- ✅ No feature flag conflicts between different FFI frameworks
- ✅ Each language adapter can evolve independently
- ✅ Easy to add new language support without modifying core
- ✅ Clearer separation of concerns

**Structure**:
```
core/
├── Cargo.toml        # Pure Rust library, crate-type = ["rlib"]
└── src/
    ├── lib.rs        # NO PyO3/NAPI imports
    ├── cli.rs        # CLI logic (used by all languages)
    └── ...

python/native/
├── Cargo.toml        # PyO3 bindings, crate-type = ["cdylib"]
└── src/lib.rs        # use currant_core::*; + PyO3 wrappers

node/native/
├── Cargo.toml        # NAPI-RS bindings, crate-type = ["cdylib"]
└── src/lib.rs        # use currant_core::*; + NAPI wrappers
```

**Never**:
- ❌ Add PyO3/NAPI dependencies to `core/Cargo.toml`
- ❌ Add language-specific code to `core/src/`
- ❌ Create separate binaries for each language

**Always**:
- ✅ Keep core as pure Rust library
- ✅ Put FFI bindings in `<lang>/native/` directories
- ✅ Have language adapters pass normalized args to Rust CLI (NOT read from process directly)

### CLI Architecture: Dual-Tier Approach

**IMPORTANT DECISION (2025-10-11)**: Two CLI options - optional global admin CLI + language adapter CLIs.

**Design Principle**: Consistency in concepts, not commands. Different languages have different idioms for CLI tools, and we embrace that.

**How Users Invoke Commands:**

| Language | Command | Version Isolation |
|----------|---------|-------------------|
| **Python** | `python -m currant <cmd>` | virtualenv |
| **Node.js** | `npx currant <cmd>` | node_modules |
| **Go** | `go run cmd/<cmd>/main.go` | go.mod |
| **Rust** | `cargo run --bin <cmd>` | Cargo.toml |
| **Ruby** | `bundle exec currant <cmd>` | Bundler |
| **Admin (any)** | `currant <cmd>` (optional) | Global install |

---

### Global Admin CLI (Optional)

**Installation:**
```bash
cargo install currant-cli
```

**Available Commands** (database operations only):
```bash
currant list           # Query executions
currant status         # Check schema status
currant cancel <id>    # Cancel execution
currant signal <id>    # Send signal to workflow
currant retry <id>     # Retry failed execution
currant cleanup        # Purge old executions
```

**NOT Available** (schema/runtime operations):
```bash
currant migrate        # ❌ Use language adapter
currant worker         # ❌ Use language adapter
currant bench          # ❌ Use language adapter
```

**Use Cases:**
- DevOps/SRE admin tasks without language runtime
- Quick debugging (`currant list --status=failed`)
- CI/CD operations (cancel, cleanup, status checks)
- Multi-language projects (one admin tool)

**Version Strategy:**
- Reads schema version from database
- Version compatibility and upgrade management: TBD (future feature)
- May offer automatic version upgrades in future iterations

---

### Language Adapter CLIs

**Complete CLI** - includes all commands from global CLI + language-specific commands.

**Python/Node.js/Ruby** (CLI-first languages):

```bash
# All commands available
python -m currant migrate      # Language-specific (uses installed package)
python -m currant worker       # Language-specific
python -m currant bench        # Language-specific
python -m currant list         # Delegates to bundled core
python -m currant cancel <id>  # Delegates to bundled core
```

**Example flow (Python)**:
```python
# python/currant/__main__.py
args = sys.argv.copy()
args[0] = 'currant'

if args[1] in ['worker', 'bench', 'migrate']:
    # Python handles language-specific commands
    await run_worker(...) or await run_bench(...) or await run_migrate()
else:
    # Rust handles: list, status, cancel, signal, retry, cleanup
    RustBridge.run_cli(args)
```

**Go/Rust** (library-first languages):

Users write their own command scripts:

```go
// cmd/migrate/main.go
package main
import "github.com/currant/currant"

func main() {
    currant.Migrate()  // Uses version from go.mod
}

// cmd/worker/main.go
func main() {
    config := currant.LoadConfigFromEnv()
    worker := currant.NewWorker(config)
    worker.Run()
}
```

```bash
go run cmd/migrate/main.go
go run cmd/worker/main.go
```

**Core provides CLI framework** (`core/src/cli.rs`):
- Command definitions and argument parsing (using `clap`)
- Implementation for database operations: `list`, `status`, `cancel`, `signal`, `retry`, `cleanup`
- Accepts `Vec<String>` args (doesn't read `std::env::args()` directly)
- Function: `pub async fn run_cli_from_args(args: Vec<String>)`

**Version Management:**
- **Language adapters**: Version tied to package (pip, npm, go.mod, Cargo.toml)
- **Global CLI**: Optional, user installs specific version or latest
- **No version coordination file needed**: Migration runs from language adapter, always uses correct version
- **Schema version**: Stored in database
- **Version compatibility**: TBD - strategy for rolling upgrades and backward compatibility

**Migration Strategy:**
- Always run `migrate` from language adapter (or custom script for Go/Rust)
- Ensures migration uses same version as application code
- Global CLI cannot run migrations (prevents version mismatches)
- Fresh database: Must initialize via language adapter first

**Rationale**:
- ✅ Language adapters are self-sufficient (no global CLI needed)
- ✅ Global CLI is optional (for ops/admin convenience)
- ✅ No version conflicts (adapters bundle compatible core)
- ✅ Migrations always match code version
- ✅ Follows language idioms (CLI-first for Python/Node, library-first for Go/Rust)
- ✅ Clear separation: schema/runtime = language, data operations = global CLI works fine

### Configuration Management

**Decision (2025-10-11)**: Use config files with environment variable overrides.

**Priority order** (highest to lowest):
1. CLI flag: `--database-url postgresql://...` (explicit override for debugging)
2. Environment variable: `CURRANT_DATABASE_URL` (container-friendly)
3. Config file: `currant.toml` (default for local dev)
4. Fallback: `postgresql://localhost/currant`

**Config file format** (TOML):
```toml
# currant.toml
[database]
url = "postgresql://workflows:workflows@localhost/workflows"

[observability]
enabled = true
endpoint = "http://localhost:4317"
```

**Config file location search order**:
1. `currant.toml` (project root, can commit to repo or gitignore for local overrides)
2. `~/.config/currant/config.toml` (user-level default)

**Optional**: Load `.env` file if present (many tools do this)

**Benefits**:
- ✅ No manual env var needed for local dev (config file "just works")
- ✅ Multi-environment support (dev/staging/prod via different configs)
- ✅ Multi-language support (Python, Node, Go all read same config file)
- ✅ Container-friendly (env vars work in Docker/K8s)
- ✅ Production debugging (can override with `--database-url`)

**Language-specific notes**:
- **Python/Node/Ruby**: Adapters load config and pass to core
- **Go/Rust**: Users can use `LoadConfigFromEnv()` helper or build their own config loading

**Example (Go):**
```go
func main() {
    // Option 1: Load from env vars (reads CURRANT_* variables)
    config := currant.LoadConfigFromEnv()

    // Option 2: Explicit
    config := currant.Config{
        DatabaseURL: "postgresql://...",
        Queues: []string{"default"},
    }

    // Option 3: Load from config file
    config := currant.LoadConfig("currant.toml")

    worker := currant.NewWorker(config)
    worker.Run()
}
```

**No version coordination file needed**: Migration always runs from language adapter (which knows its own version), so no `.currant/config.toml` version field is required.

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

## Performance and Scalability

### Worker Polling vs LISTEN/NOTIFY

**Current Approach**: Poll-based worker loop (0.05-0.1s intervals)

**Decision**: For initial release, use optimized polling instead of LISTEN/NOTIFY

**Rationale**:
- **FFI Complexity**: Exposing Rust's async `PgListener.recv()` across FFI boundaries is complex
  - Blocking calls interfere with language async runtimes (asyncio, Node event loop)
  - Would need per-language async FFI integration (complicated, error-prone)
- **Performance is Acceptable**:
  - 50ms polling = ~25ms average latency (fine for most workloads)
  - 100 workers = 2,000 QPS when idle (Postgres can handle 10k-50k QPS easily)
  - Breakpoint: ~500-1000 workers before polling becomes a bottleneck
- **Simpler Architecture**:
  - Language owns the worker loop and async concurrency control
  - Rust provides simple synchronous FFI functions
  - Works consistently across all language runtimes

**Trade-offs**:
- ✅ Simple, works across all languages without runtime-specific integration
- ✅ Good enough for <100 workers (99% of users)
- ⚠️ Not optimal for 1000+ worker deployments
- ⚠️ Higher latency (~25ms vs <1ms with LISTEN/NOTIFY)

**Future**: For users needing <10ms latency or running 1000+ workers:
- Provide optional Rust binary worker that uses native LISTEN/NOTIFY
- Calls language functions via subprocess/HTTP/gRPC
- Advanced users opt-in to this complexity

### Benchmarking Tool

**Decision (Updated 2025-10-11)**: Benchmarking is implemented in language adapters, not core.

**Implementation**:
- Each language adapter provides `bench` command (e.g., `python -m currant bench`)
- Adapter handles spawning workers (knows correct invocation for that language)
- Benchmark functions (`__currant_bench_*`) ship in adapter's benchmark module
- Adapter enqueues jobs/workflows via Rust FFI, workers execute, adapter queries metrics from DB

**Why adapter-specific**:
- Tests the **full stack**: FFI overhead, serialization, async scheduling, database
- **Language-specific by design**: Validates that specific adapter's performance
- Adapter knows how to spawn its own workers correctly
- Core stays language-agnostic (no spawning logic, no language assumptions)
- Users benchmark the actual setup they'll deploy

**Benchmark jobs**:
- `__currant_bench_noop__`: Minimal overhead job (tests throughput)
- `__currant_bench_compute__`: CPU-bound job (tests under load)
- `__currant_bench_activity__`: No-op activity (tests workflow coordination)
- `__currant_bench_workflow__`: Workflow with N activities (tests end-to-end)

**Usage**:
```bash
# Python
python -m currant bench --workers 10 --jobs 1000

# Node
npx currant bench --workers 10 --workflows 100 --activities-per-workflow 5

# Go/Rust (library-only, users can write custom benchmarks if needed)
```

**Metrics collected**:
- Jobs/sec throughput
- Average latency (created_at → completed_at)
- Success/failure rates
- Database query load
- Worker utilization

## Common Patterns

### Adding a New Language Adapter

Currant is designed to support any language with FFI capabilities. To add a new language:

1. Create `<lang>/native/` directory with FFI bindings to Rust core
   - Python uses PyO3, Node.js uses NAPI-RS, Go would use CGO, etc.
2. Implement decorators/annotations for function registration
3. Implement worker loop: claim → execute → report
4. Implement workflow replay logic (handle suspend/resume)
5. Implement client API (`.queue()`, `send_signal()`)
6. Add benchmark module with language-specific benchmark functions

### Debugging Workflow Issues

- Check `executions` table for state
- Look at `checkpoint` JSON for replay history
- Worker logs show which activities executed
- Use `get_workflow_activities()` to see child executions

### Running Benchmarks

```bash
# Measure your setup's performance
currant bench --workers 10 --jobs 1000 --workflows 100

# Test different queue configurations
currant bench --workers 20 --queues default,priority,background

# Test with realistic payloads
currant bench --workers 10 --jobs 1000 --payload-size 10000
```

---

**Remember**: Update this file when making significant architectural decisions!
