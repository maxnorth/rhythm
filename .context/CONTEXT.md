# Rhythm - Project Context

> **Instructions for Claude**: Read this file at the start of each session (via `/context` command). After making significant architectural decisions or design changes, UPDATE this file to preserve context for future conversations.

## Development Status

**Pre-release**: Rhythm has not been released yet and has no users.
**Breaking changes are OK** - Do not add backwards compatibility code or worry about breaking changes. Make clean, direct changes.

## Performance Requirements

⚠️ **CRITICAL**: This project has exceptionally high performance expectations. Performance must be considered at all times during design and implementation.

**Performance Philosophy**:
- Database operations are the bottleneck - minimize roundtrips
- Use PostgreSQL's atomic operations (`INSERT ... ON CONFLICT`, `UPDATE ... RETURNING`, etc.) over application-level logic
- Prefer single-query solutions over multi-query transactions where possible
- Consider connection pool exhaustion and latency implications
- Race conditions from multi-query patterns can impact correctness AND performance

**When implementing features**:
1. Always consider the performance implications of your approach
2. Use database-level atomic operations when available
3. Minimize network roundtrips between application and database
4. Test under realistic concurrent load conditions
5. Benchmark critical paths before and after changes

**Target Performance Metrics** (baseline mode, pure Rust):
- 2000-10000+ tasks/sec throughput
- <10ms P99 latency for task claiming
- Minimal connection pool contention
- Scalable to 100+ concurrent workers

**When in doubt**: Ask about performance implications before implementing. It's easier to design for performance upfront than to optimize later.

## Recent Changes

**2024-10-12**: Simplified execution types to only "task" and "workflow"
- Removed "task" and "task" types entirely - consolidated into "task"
- `@task` decorator for both standalone and workflow steps
- Tasks with `parent_workflow_id` are workflow steps
- Database migration consolidated all types to "task" and "workflow"
- All code, docs, and examples updated throughout codebase
- No backwards compatibility needed (pre-release)

## What is Rhythm?

Rhythm is a **lightweight durable execution framework** that enables building reliable, multi-step workflows using only PostgreSQL - no external orchestrator needed.

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
│  - Decorators (@task)           │
│  - Worker loops                 │
│  - DSL workflow integration     │
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
│  - workflow_definitions         │
│  - workflow_execution_context   │
│  - workflow_signals             │
│  - pg_notify channels           │
└─────────────────────────────────┘
```

### Key Components

**Rust Core** (`/core`):
- `lib.rs` - PyO3/FFI bindings exposing Rust functions to language adapters
- `db.rs` - Database connection pooling (sqlx)
- `executions.rs` - CRUD operations for tasks/workflows
- `worker.rs` - Worker heartbeat, dead worker detection, failover
- `signals.rs` - Workflow signal management
- `types.rs` - Shared data structures

**Python Adapter** (`/python/rhythm`):
- `decorators.py` - `@task` decorator
- `registry.py` - Function registry for decorated functions
- `worker.py` - Worker loop (claim → execute → report)
- `client.py` - `start_workflow()`, `send_signal()` client API
- `init.py` - DSL workflow registration
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
- **Rhythm**: Only Postgres

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

### 4. Workflow Execution Models

**Decision**: Support two workflow models - DSL-based (recommended) and Python replay (legacy).

#### DSL-Based Workflows (Recommended)

**How it works**:
1. Workflows written in `.flow` files with simple orchestration syntax
2. Rust core parses DSL to AST, stores in `workflow_definitions` table with cached JSON
3. Tree-walking interpreter executes statement by statement
4. State is flat: `{statement_index, locals, awaiting_task_id}`
5. On `task()`: Creates child execution, suspends workflow, saves state
6. On task completion: Workflow re-enters queue, resumes from next statement
7. No replay - just continue from saved position

**Why this approach**:
- ✅ Language-agnostic (same DSL works with Python, Node.js, any language)
- ✅ Simpler state management (flat state, no call stack)
- ✅ Inherently deterministic (limited DSL prevents non-determinism)
- ✅ Easier to visualize as DAG
- ✅ No replay complexity
- ⚠️ Limited expressiveness initially (no conditionals/loops yet - planned)

**Current syntax**:
```
task("taskName", { "arg": "value" })
sleep(5)
```

**See**: [DSL_WORKFLOW_IMPLEMENTATION.md](/Users/maxnorth/Projects/rhythm/.context/DSL_WORKFLOW_IMPLEMENTATION.md)

## Execution Model

### Two Execution Types

1. **Task**: Async unit of work
   - Can run standalone (via `.queue()`)
   - Can run as workflow step (from DSL workflow or via `.run()` inside Python workflow)
   - Distinguished by `parent_workflow_id` column:
     - NULL = standalone task
     - Set = workflow step
   - Retries on failure
   - Example: Send email, charge payment, validate order

2. **Workflow**: Multi-step orchestration
   - **DSL workflows** (recommended): Written in `.flow` files, executed by tree-walking interpreter
   - **Python workflows** (legacy): Decorated functions with deterministic replay
   - Coordinates multiple tasks
   - Survives crashes via state persistence
   - Example: Order processing, approval flow

### Queue-First Design

**Everything is queued by default**. No synchronous execution model.

```python
# Enqueue (returns immediately with execution_id)
task_id = await send_email.queue(to="user@example.com", subject="Hi")

# Worker picks it up asynchronously
```

**Why**: Decouples producers from workers, enables scaling, built-in reliability.

## Current State (as of 2025-10-20)

### ✅ Implemented
- Rust core with full execution management
- Python adapter (complete, tested, all scripts verified)
- Node.js adapter (complete, native bindings working, 23 tests passing)
- Worker coordination and failover
- Workflow signals
- Versioning support (for Python workflows)
- CLI tools
- Migrations
- **DSL-based workflows** (basic implementation):
  - Parser for `.flow` files
  - Workflow registration from filesystem
  - Tree-walking interpreter
  - `task()` and `sleep()` statements (sleep not yet scheduled)
  - Worker integration (auto-detects DSL vs Python workflows)
  - Flat state serialization
  - End-to-end working example

### 📋 Future Roadmap
- **DSL completion**:
  - Control flow (if/else, loops)
  - Expressions and operators
  - Sleep scheduling implementation
  - Error handling
  - Standard library helpers
  - Better error messages
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
- **Python**: `python/native/` - PyO3 bindings that import `rhythm-core`
- **Node.js**: `node/native/` - NAPI-RS bindings that import `rhythm-core`
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
└── src/lib.rs        # use rhythm_core::*; + PyO3 wrappers

node/native/
├── Cargo.toml        # NAPI-RS bindings, crate-type = ["cdylib"]
└── src/lib.rs        # use rhythm_core::*; + NAPI wrappers
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
| **Python** | `python -m rhythm <cmd>` | virtualenv |
| **Node.js** | `npx rhythm <cmd>` | node_modules |
| **Go** | `go run cmd/<cmd>/main.go` | go.mod |
| **Rust** | `cargo run --bin <cmd>` | Cargo.toml |
| **Ruby** | `bundle exec rhythm <cmd>` | Bundler |
| **Admin (any)** | `rhythm <cmd>` (optional) | Global install |

---

### Global Admin CLI (Optional)

**Installation:**
```bash
cargo install rhythm-cli
```

**Available Commands** (database operations only):
```bash
rhythm list           # Query executions
rhythm status         # Check schema status
rhythm cancel <id>    # Cancel execution
rhythm signal <id>    # Send signal to workflow
rhythm retry <id>     # Retry failed execution
rhythm cleanup        # Purge old executions
```

**NOT Available** (schema/runtime operations):
```bash
rhythm migrate        # ❌ Use language adapter
rhythm worker         # ❌ Use language adapter
rhythm bench          # ❌ Use language adapter
```

**Use Cases:**
- DevOps/SRE admin tasks without language runtime
- Quick debugging (`rhythm list --status=failed`)
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
python -m rhythm migrate      # Language-specific (uses installed package)
python -m rhythm worker       # Language-specific
python -m rhythm bench        # Language-specific
python -m rhythm list         # Delegates to bundled core
python -m rhythm cancel <id>  # Delegates to bundled core
```

**Example flow (Python)**:
```python
# python/rhythm/__main__.py
args = sys.argv.copy()
args[0] = 'rhythm'

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
import "github.com/rhythm/rhythm"

func main() {
    rhythm.Migrate()  // Uses version from go.mod
}

// cmd/worker/main.go
func main() {
    config := rhythm.LoadConfigFromEnv()
    worker := rhythm.NewWorker(config)
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

**Decision (2025-10-11, Implemented 2025-10-12)**: Use config files with environment variable overrides. Database URL is **required** - no default provided.

**Implementation**: Core provides `core/src/config.rs` module that handles all configuration loading.

**Priority order** (highest to lowest):
1. CLI flag: `--database-url postgresql://...` (explicit override for debugging)
2. Environment variable: `RHYTHM_DATABASE_URL` (container-friendly)
3. Config file: `rhythm.toml` (default for local dev)
4. **Required**: Database URL must be set via one of the above methods

**Why no default URL?** Following best practices from Temporal, Celery, and other production tools:
- Prevents accidental connections to wrong databases
- Forces explicit configuration (fail fast)
- Environment-specific values shouldn't have defaults
- Clear error message guides users to proper setup

**Config file format** (TOML):
```toml
# rhythm.toml
[database]
url = "postgresql://localhost/rhythm"
max_connections = 50
min_connections = 5
acquire_timeout_secs = 10
idle_timeout_secs = 600
max_lifetime_secs = 1800
```

**All configurable settings**:
- `database.url`: PostgreSQL connection string
- `database.max_connections`: Pool max connections (default: 50)
- `database.min_connections`: Pool min connections (default: 5)
- `database.acquire_timeout_secs`: Connection acquire timeout (default: 10)
- `database.idle_timeout_secs`: Idle connection timeout (default: 600)
- `database.max_lifetime_secs`: Max connection lifetime (default: 1800)

**Environment variable mapping**:
- `RHYTHM_DATABASE_URL` → `database.url`
- `RHYTHM_DATABASE_MAX_CONNECTIONS` → `database.max_connections`
- `RHYTHM_DATABASE_MIN_CONNECTIONS` → `database.min_connections`
- `RHYTHM_DATABASE_ACQUIRE_TIMEOUT_SECS` → `database.acquire_timeout_secs`
- `RHYTHM_DATABASE_IDLE_TIMEOUT_SECS` → `database.idle_timeout_secs`
- `RHYTHM_DATABASE_MAX_LIFETIME_SECS` → `database.max_lifetime_secs`
- `RHYTHM_CONFIG_PATH` → Override config file location

**Config file location search order**:
1. `RHYTHM_CONFIG_PATH` env var (if set)
2. `rhythm.toml` (project root, can commit to repo or gitignore for local overrides)
3. `~/.config/rhythm/config.toml` (user-level default)

**`.env` file support**: Automatically loaded if present in project root (using `dotenvy` crate)

**Benefits**:
- ✅ No manual env var needed for local dev (config file "just works")
- ✅ Multi-environment support (dev/staging/prod via different configs)
- ✅ Multi-language support (Python, Node, Go can all read same TOML file)
- ✅ Container-friendly (env vars work in Docker/K8s)
- ✅ Production debugging (can override with `--database-url`)
- ✅ All settings are configurable at every layer

**Usage in Rust**:
```rust
use rhythm_core::config::Config;

// Load with full priority chain
let config = Config::load()?;

// Load from specific file
let config = Config::from_file("rhythm.toml")?;

// Use builder for programmatic overrides
let config = Config::builder()
    .database_url(Some("postgresql://...".to_string()))
    .max_connections(Some(100))
    .build()?;
```

**Global CLI integration**:
```bash
# Use default config search
rhythm bench --tasks 1000

# Override config file location
rhythm --config /path/to/rhythm.toml bench --tasks 1000

# Override database URL directly
rhythm --database-url postgresql://prod/db bench --tasks 1000
```

**Future extensibility pattern**:
When adding new features (e.g., observability), add new config sections:
```toml
[observability]
endpoint = "http://localhost:4317"
sample_rate = 0.1
```

Then update `core/src/config.rs`:
1. Add new struct (e.g., `ObservabilityConfig`)
2. Add field to `Config` struct: `pub observability: Option<ObservabilityConfig>`
3. Add env var mappings in `apply_env_vars()`
4. Add CLI flags if needed (e.g., `--observability-endpoint`)

**No version coordination file needed**: Migration always runs from language adapter (which knows its own version), so no `.rhythm/config.toml` version field is required.

### Testing Philosophy

- Rust core: Unit tests + integration tests with test database
- Adapters: Function registry, worker loop, replay logic
- E2E: Full worker + enqueue scenarios

### Database Schema

Critical tables:
- `executions`: All tasks/workflows (polymorphic)
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

**Decision (Updated 2025-10-12)**: Two benchmark modes - baseline (pure Rust) and adapter (full stack).

#### Architecture Overview

**Core provides unified benchmark implementation** (`core/src/benchmark.rs`):
- Single orchestration logic: enqueue work → wait for completion → collect metrics
- Two worker modes:
  1. **Baseline mode**: Internal tokio tasks (pure Rust, no external processes)
  2. **Adapter mode**: External worker processes (spawned via command passed by adapter)
- Core stays language-agnostic (adapters tell core how to spawn their workers)

#### Baseline Benchmark (Admin CLI)

**Purpose**: Establish performance ceiling - test pure database/core throughput without FFI/serialization/language runtime overhead.

**How it works**:
- Spawns N concurrent tokio tasks (not processes) in the benchmark process
- Each task runs tight loop: `claim_execution()` → optional delay → `complete_execution()`
- Uses tokio multi-threaded runtime to maximize CPU utilization
- No worker processes, no FFI, no serialization - pure Rust throughput

**Command**:
```bash
# Admin CLI (also available via Python passthrough)
rhythm bench --tasks 10000 --concurrency 1000

# Optional: simulate work between claim/complete
rhythm bench --tasks 10000 --concurrency 1000 --work-delay-us 100
```

**Configuration**:
- `--concurrency N`: Number of concurrent claim loops (tokio tasks, not processes)
- `--work-delay-us N`: Microseconds of simulated work per task (default: 0 = pure DB load)
- `--tasks N`: Number of tasks to enqueue
- `--workflows N`: Number of workflows to enqueue
- `--duration 30s`: Max benchmark duration

**Use cases**:
- "Can the database handle X tasks/sec?"
- "What's the theoretical maximum throughput?"
- "Is the bottleneck in core or in the language adapter?"

#### Adapter Benchmark (Language-Specific)

**Purpose**: Measure real-world performance including FFI, serialization, language runtime, and worker coordination.

**How it works**:
- Language adapter tells core how to spawn workers (passes command as argument)
- Core spawns N worker processes using the provided command
- Workers claim and execute actual benchmark functions
- Tests full stack: FFI overhead, serialization, async scheduling, database

**Commands**:
```bash
# Python - baseline (passthrough to admin CLI)
python -m rhythm bench --tasks 10000 --concurrency 1000
# ⚠️  Warning: Running baseline benchmark (pure Rust)
#    To benchmark Python workers: python -m rhythm bench worker --workers N

# Python - worker benchmark (Python-specific command)
python -m rhythm bench worker --workers 10 --tasks 1000

# Node.js - worker benchmark
npx rhythm bench worker --workers 10 --tasks 1000
```

**Benchmark functions** (in adapter modules):
- `__rhythm_bench_noop__`: Minimal overhead task (tests throughput)
- `__rhythm_bench_compute__`: CPU-bound task (tests under load)
- `__rhythm_bench_task__`: No-op task (tests workflow coordination)
- `benchWorkflow`: DSL workflow dynamically generated with N tasks (tests end-to-end DSL execution)

**Configuration**:
- `--workers N`: Number of worker processes to spawn
- `--tasks N`: Number of tasks to enqueue
- `--workflows N`: Number of workflows to enqueue
- Other options: `--payload-size`, `--compute-iterations`, `--duration`, etc.

**Use cases**:
- "What throughput will my Python deployment achieve?"
- "How much overhead does FFI/serialization add?"
- "Performance comparison: Python vs Node.js vs future Rust adapter"

#### Comparison & Interpretation

**Expected performance relationships**:
- **Baseline**: 2000-10000+ tasks/sec (pure database ceiling)
- **Python adapter**: 20-30% of baseline (FFI + GIL + serialization overhead)
- **Node.js adapter**: 30-40% of baseline (FFI + event loop overhead)
- **Future Rust adapter**: 80-90% of baseline (minimal FFI, native async)

**Example analysis**:
```bash
# Run baseline
rhythm bench --tasks 10000 --concurrency 1000
# Result: 5000 tasks/sec

# Run Python worker benchmark
python -m rhythm bench worker --workers 10 --tasks 10000
# Result: 1200 tasks/sec (24% of baseline)

# Interpretation: 76% overhead from Python (FFI, serialization, GIL)
# This is expected and acceptable for Python deployments
```

#### Metrics Collected (Same for Both Modes)

- Tasks/sec throughput
- Latency percentiles (P50, P95, P99) - created_at → completed_at
- Success/failure/pending counts
- Separate metrics for tasks vs workflows
- Warmup period support (exclude first N% from stats)

#### Implementation Details

**Core benchmark orchestration** (`core/src/benchmark.rs`):
```rust
pub enum WorkerMode {
    Baseline { concurrency: usize, work_delay_us: Option<u64> },
    External { command: Vec<String>, workers: usize },
}

pub async fn run_benchmark(mode: WorkerMode, params: BenchmarkParams) -> Result<()> {
    // Start workers (either internal tasks or external processes)
    // Enqueue work to database
    // Wait for completion
    // Collect metrics from database
    // Display report
}
```

**Language adapter integration** (Python example):
```python
# python/rhythm/__main__.py
if args[1] == 'bench':
    if len(args) > 2 and args[2] == 'worker':
        # Python handles: bench worker
        worker_cmd = ["python", "-m", "rhythm", "worker", "--queue", "default", "--import", "rhythm.benchmark"]
        RustBridge.run_benchmark(
            mode=WorkerMode::External(worker_cmd),
            workers=args.workers,
            params=...
        )
    else:
        # Passthrough to admin CLI: bench (baseline mode)
        print("⚠️  Running baseline benchmark (pure Rust)")
        print("   To benchmark Python workers: python -m rhythm bench worker --workers N")
        RustBridge.run_cli(args)
```

#### Design Rationale

**Why two modes?**
- Baseline establishes theoretical ceiling and validates database performance
- Adapter benchmarks show real-world deployment performance
- Comparison reveals where optimization efforts should focus

**Why core orchestrates both?**
- Single implementation of metrics collection (consistency)
- Same output format (easy comparison)
- Core stays language-agnostic (adapters just pass worker command)

**Why baseline uses tokio tasks not processes?**
- Maximizes throughput (1000s of concurrent operations)
- Tests pure database capacity without process spawn overhead
- Represents best-case performance (what a highly-optimized Rust adapter could achieve)

## Common Patterns

### Adding a New Language Adapter

Rhythm is designed to support any language with FFI capabilities. To add a new language:

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
- Worker logs show which tasks executed
- Use `get_workflow_tasks()` to see child executions

### Running Benchmarks

```bash
# Baseline benchmark (pure Rust, theoretical maximum)
rhythm bench --tasks 10000 --concurrency 1000

# Baseline with simulated work
rhythm bench --tasks 10000 --concurrency 1000 --work-delay-us 100

# Python worker benchmark (full stack)
python -m rhythm bench worker --workers 10 --tasks 1000

# Test with realistic payloads
python -m rhythm bench worker --workers 10 --tasks 1000 --payload-size 10000

# Test different queue configurations
python -m rhythm bench worker --workers 20 --queues default,priority
```

---

**Remember**: Update this file when making significant architectural decisions!
