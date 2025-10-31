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
│   │   ├── workflows.rs     # DSL workflow registration
│   │   └── interpreter/     # DSL parser & executor
│   ├── migrations/          # SQL migrations
│   └── Cargo.toml
│
├── python/            # Python adapter
│   └── rhythm/
│       ├── rust_bridge.py   # Rust FFI wrapper
│       ├── decorators.py    # @task decorator
│       ├── client.py        # start_workflow(), send_signal()
│       ├── worker.py        # Python worker loop
│       └── init.py          # DSL workflow registration
│
└── examples/          # Example applications
    └── workflow_example/    # DSL workflow example
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
- `workflows.rs`: DSL workflow registration and starting
- `interpreter/`: DSL parser and execution engine
  - `parser.rs`: Parse `.flow` files to AST
  - `executor.rs`: Tree-walking interpreter for workflow execution
- `types.rs`: Shared data structures
- `lib.rs`: PyO3 bindings exposing Rust functions to Python

### Python Adapter Responsibilities

The Python adapter (`python/`) provides an idiomatic Python API:

- **Decorators**: `@task` for defining async tasks
- **Function Registry**: Track decorated functions for execution
- **Worker Loop**: Claim from Rust → Execute Python task or delegate DSL workflow to Rust → Report back
- **DSL Workflow Integration**: Delegates workflow execution to Rust core
- **Serialization**: Convert Python args/kwargs/results to/from JSON

**Key modules:**
- `rust_bridge.py`: Thin wrapper around Rust FFI, handles JSON serialization
- `decorators.py`: `@task` decorator, function registry
- `client.py`: Client API (`start_workflow()`, `send_signal()`)
- `worker.py`: Worker loop implementation
- `init.py`: DSL workflow registration helper

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

  2. Check execution type:
     - If task: Execute Python function
     - If workflow: Delegate to Rust (DSL workflow)

  3. On success:
     Call RustBridge.complete_execution(execution_id, result)

  4. On failure:
     Call RustBridge.fail_execution(execution_id, error, retry)
```

**DSL Workflow Execution:**
```
Worker claims DSL workflow execution:
  1. Worker detects execution has no Python function (DSL workflow)
  2. Worker calls RustBridge.execute_workflow_step(execution_id)
     → Rust loads workflow_execution_context (statement_index, locals, awaiting_task_id)
     → Rust loads cached parsed_steps from workflow_definitions table
     → Rust executes current statement:
        - task(): Creates child task execution, suspends workflow
        - sleep(): Schedules timer (TODO), advances to next statement
     → Rust saves updated state (statement_index++, locals)
  3. If suspended: workflow waits for child task completion
  4. If completed: workflow marked complete
  5. When child task completes: workflow re-enters queue
  6. Worker claims workflow again, resumes from next statement
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
python -m rhythm worker -q orders -q notifications -m examples.simple_example

# Terminal 2: Enqueue work
python examples/enqueue_example.py
```

## Workflows

Rhythm uses DSL-based workflows written in `.flow` files.

**Example** (`processOrder.flow`):
```
task("chargeCard", { "orderId": "order-123", "amount": 99.99 })
sleep(5)
task("shipOrder", { "orderId": "order-123" })
task("sendEmail", { "to": "customer@example.com", "subject": "Shipped!" })
```

**Benefits**:
- Language-agnostic (same DSL works with Python, Node.js, any adapter)
- Simple flat state serialization (just `{statement_index, locals, awaiting_task_id}`)
- Inherently deterministic (limited DSL prevents non-determinism)
- Easy to visualize and debug
- Cached parsing (parse once, store JSON in database)

**Execution**:
- Rust core parses `.flow` files to AST, stores in `workflow_definitions` table
- Tree-walking interpreter executes statement by statement
- State saved after each step (statement_index, local variables)
- Suspends when awaiting tasks, resumes when tasks complete
- No replay needed - just continue from saved position

**Current syntax**:
- `task("taskName", { "arg": "value" })` - Execute a task
- `sleep(seconds)` - Sleep (not yet implemented)

**Planned features**:
- Conditionals: `if (result.success) { ... }`
- Loops: `for (item in items) { ... }`
- Expressions: Variables, operators, property access

See [DSL_WORKFLOW_IMPLEMENTATION.md](/Users/maxnorth/Projects/rhythm/.context/DSL_WORKFLOW_IMPLEMENTATION.md) for complete implementation details.

---

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
