# Migration to Rust Core

This document tracks the migration from pure Python to Rust core + Python adapter architecture.

## Removed Files

### `python/workflows/db.py` (REMOVED)
**Reason:** Database connection pooling and migration logic moved to Rust core.

**Replaced by:**
- `core/src/db.rs` - Rust database module with sqlx
- `RustBridge.migrate()` - Python FFI wrapper

**Previous functionality:**
- `get_connection()` - AsyncPG connection
- `get_pool()` - AsyncPG connection pool
- `run_migrations()` - SQL schema execution
- `close_pool()` - Pool cleanup

### `python/workflows/schema.sql` (REMOVED)
**Reason:** SQL schema moved to Rust migrations.

**Replaced by:**
- `core/migrations/20241005000001_initial_schema.sql`
- Managed by sqlx migrations in Rust

## Modified Files

### `python/workflows/cli.py`
- **Before:** `from workflows.db import run_migrations, close_pool`
- **After:** `from workflows.rust_bridge import RustBridge`
- **Change:** `migrate()` command now calls `RustBridge.migrate()`

### `python/workflows/client.py`
- **Before:** Direct AsyncPG database operations
- **After:** All operations go through `RustBridge`
- **Changes:**
  - `queue_execution()` → `RustBridge.create_execution()`
  - `send_signal()` → `RustBridge.send_signal()`
  - `get_execution_status()` → `RustBridge.get_execution()`
  - `cancel_execution()` → `RustBridge.fail_execution()`

### `python/workflows/worker.py`
- **Before:** Direct AsyncPG database operations for all worker coordination
- **After:** All database operations through `RustBridge`
- **Changes:**
  - `_claim_execution()` → `RustBridge.claim_execution()`
  - `_complete_execution()` → `RustBridge.complete_execution()`
  - `_handle_execution_failure()` → `RustBridge.fail_execution()`
  - `_update_heartbeat()` → `RustBridge.update_heartbeat()`
  - `_recover_dead_worker_executions()` → `RustBridge.recover_dead_workers()`
  - `_handle_workflow_suspend()` → `RustBridge.suspend_workflow()` + `create_execution()`
  - `_resume_parent_workflow()` → `RustBridge.get_execution()` + `suspend_workflow()` + `resume_workflow()`
  - `stop()` → `RustBridge.stop_worker()`

### `python/workflows/models.py`
- **Added:** `Execution.from_dict()` classmethod to deserialize Rust JSON responses

## What Stays in Python

### Core Functionality (Still Python)
- `decorators.py` - `@job`, `@activity`, `@workflow` decorators
- `registry.py` - Function registry for decorated functions
- `context.py` - Workflow execution context and replay logic
- `worker.py` - Worker loop orchestration and function execution
- `config.py` - Configuration management
- `utils.py` - Python utility functions
- `models.py` - Pydantic data models

### Why These Stay in Python
1. **Decorators** - Language-specific syntax
2. **Function Registry** - Needs access to Python runtime to call decorated functions
3. **Workflow Context** - Handles `WorkflowSuspendException` for replay (language-specific)
4. **Worker Loop** - Orchestrates Python async function execution
5. **Models** - Used for validation and type hints in Python code

## Architecture Summary

```
┌─────────────────────────────────────────┐
│           Python Layer                  │
│  ┌──────────────────────────────────┐  │
│  │ Decorators (@job, @workflow)     │  │
│  │ Function Registry                │  │
│  │ Workflow Replay Logic            │  │
│  │ Function Execution               │  │
│  └──────────────┬───────────────────┘  │
│                 │                       │
│  ┌──────────────▼───────────────────┐  │
│  │     RustBridge (PyO3 FFI)        │  │
│  └──────────────┬───────────────────┘  │
└─────────────────┼───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│           Rust Core                     │
│  ┌──────────────────────────────────┐  │
│  │ Database Operations (sqlx)       │  │
│  │ Connection Pooling               │  │
│  │ Execution Management             │  │
│  │ Worker Coordination              │  │
│  │ Heartbeats & Failover            │  │
│  │ LISTEN/NOTIFY                    │  │
│  └──────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

## Benefits of Migration

1. **Performance** - Rust handles all database I/O with connection pooling
2. **Type Safety** - Compile-time checked SQL queries with sqlx
3. **Correctness** - Rust's ownership system prevents data races
4. **Polyglot Ready** - Core logic is language-agnostic, easy to add TypeScript/Go adapters
5. **Single Source of Truth** - No need to duplicate execution logic across languages
6. **Better Testing** - Rust's test framework for core logic, language tests for adapters

## Migration Checklist

- [x] Move database operations to Rust
- [x] Move migrations to Rust (sqlx)
- [x] Create PyO3 bindings
- [x] Update Python client to use Rust
- [x] Update Python worker to use Rust
- [x] Update CLI to use Rust migrations
- [x] Remove deprecated Python files
- [x] Update .gitignore for Rust artifacts
- [ ] Test end-to-end workflow execution
- [ ] Add Rust tests
- [ ] Update documentation
