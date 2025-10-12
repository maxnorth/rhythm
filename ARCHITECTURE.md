# Workflows Architecture

This project implements a durable execution framework with a Rust core and language-specific adapters.

## Structure

```
workflows/
├── core/              # Rust core engine
│   ├── src/
│   │   ├── lib.rs           # PyO3 bindings
│   │   ├── types.rs         # Core types
│   │   ├── db.rs            # Database pooling
│   │   ├── executions.rs    # Execution management
│   │   ├── worker.rs        # Worker coordination
│   │   └── signals.rs       # Workflow signals
│   ├── migrations/          # SQL migrations
│   └── Cargo.toml
│
├── python/            # Python adapter
│   └── currant/
│       ├── rust_bridge.py   # Rust FFI wrapper
│       ├── decorators.py    # @task, @workflow
│       ├── client.py        # .queue(), send_signal()
│       ├── worker.py        # Python worker loop
│       └── context.py       # Workflow context
│
└── examples/          # Example applications
    ├── simple_example.py
    ├── signal_example.py
    └── enqueue_example.py
```

## Architecture

### Rust Core Responsibilities

The Rust core (`core/`) handles all heavy lifting:

- **Database Operations**: Connection pooling, migrations, all SQL queries
- **Execution Management**: Claiming, completing, failing, suspending/resuming
- **Worker Coordination**: Heartbeats, dead worker detection, failover
- **Signals**: Sending and receiving workflow signals
- **LISTEN/NOTIFY**: PostgreSQL pub/sub for instant task pickup

**Key modules:**
- `db.rs`: Database connection pool using sqlx
- `executions.rs`: All execution CRUD operations
- `worker.rs`: Worker heartbeat and failover logic
- `signals.rs`: Workflow signal management
- `types.rs`: Shared data structures
- `lib.rs`: PyO3 bindings exposing Rust functions to Python

### Python Adapter Responsibilities

The Python adapter (`python/`) provides an idiomatic Python API:

- **Decorators**: `@task`, `@workflow`
- **Function Registry**: Track decorated functions for execution
- **Worker Loop**: Claim from Rust → Execute Python function → Report back to Rust
- **Workflow Replay**: Handle `WorkflowSuspendException` for checkpointing
- **Context Management**: `wait_for_signal()`, `get_version()`, workflow context
- **Serialization**: Convert Python args/kwargs/results to/from JSON

**Key modules:**
- `rust_bridge.py`: Thin wrapper around Rust FFI, handles JSON serialization
- `decorators.py`: Decorator implementations, function registry
- `client.py`: Client API (`.queue()`, `send_signal()`)
- `worker.py`: Worker loop implementation
- `context.py`: Workflow execution context

### Communication Flow

**Enqueuing Work:**
```
Python decorator.queue()
  → RustBridge.create_execution()
    → Rust: Insert into executions table
    → Rust: NOTIFY queue channel
```

**Worker Execution:**
```
Python Worker Loop:
  1. Call RustBridge.claim_execution(worker_id, queues)
     → Rust: SELECT ... FOR UPDATE SKIP LOCKED
     → Returns execution JSON

  2. Deserialize and execute Python function

  3. If WorkflowSuspendException raised:
     - Call RustBridge.suspend_workflow(workflow_id, checkpoint)
     - Create child task execution
     - Continue loop

  4. On success:
     Call RustBridge.complete_execution(execution_id, result)

  5. On failure:
     Call RustBridge.fail_execution(execution_id, error, retry)
```

**Workflow Replay:**
```
Workflow calls task.run()
  → Python raises WorkflowSuspendException
  → Worker catches it
  → Worker calls Rust to create task execution and suspend workflow
  → Task completes (separate execution)
  → Worker calls Rust to resume workflow
  → Workflow re-executes from beginning
  → Previous tasks return cached results from checkpoint
  → Workflow continues to next task or completes
```

## Building

### Prerequisites

- Rust toolchain (rustup.rs)
- Python 3.8+
- PostgreSQL 14+
- maturin (`pip install maturin`)

### Build Rust Core

```bash
cd core
maturin develop  # Builds and installs Python extension in dev mode
```

### Install Python Package

```bash
cd python
pip install -e .
```

### Run Migrations

```bash
export WORKFLOWS_DATABASE_URL="postgresql://localhost/workflows"
python -c "from workflows.rust_bridge import RustBridge; RustBridge.migrate()"
```

## Testing

### Rust Tests

```bash
cd core
cargo test
```

### Python Tests

```bash
cd python
pytest
```

### End-to-End Test

```bash
# Terminal 1: Start worker
export WORKFLOWS_DATABASE_URL="postgresql://localhost/workflows"
python -m currant worker -q orders -q notifications -m examples.simple_example

# Terminal 2: Enqueue work
python examples/enqueue_example.py
```

## Design Decisions

### Why Rust Core + Language Adapters?

1. **Performance**: Rust handles all database operations, worker coordination, and heavy lifting
2. **Correctness**: Rust's type system and ownership model prevent entire classes of bugs
3. **Polyglot**: Core logic implemented once, language adapters are thin wrappers
4. **Testability**: Core engine thoroughly tested in Rust, adapters handle language-specific concerns

### Why Not Pure Language Implementations?

- **Code Duplication**: Would need to reimplement core logic in every language
- **Inconsistent Behavior**: Subtle differences between implementations
- **Maintenance Burden**: Bug fixes and features need to be implemented N times

### Rust vs Go/C++/etc?

- **Rust**: Best balance of performance, safety, and FFI support (PyO3, neon for Node.js, etc.)
- **No GC pauses**: Critical for worker coordination and database operations
- **sqlx**: Compile-time checked SQL queries
- **Cargo**: Excellent build system and dependency management

### Why PyO3 Instead of HTTP/gRPC?

- **Performance**: Function calls are ~100x faster than network round-trips
- **Simplicity**: No separate server process, easier deployment
- **Type Safety**: PyO3 provides type-safe FFI boundary

## Future Work

- [ ] TypeScript/Node.js adapter
- [ ] Go adapter
- [ ] Workflow versioning support in Rust core
- [ ] Distributed tracing integration
- [ ] Metrics and observability
- [ ] Workflow visualization dashboard
