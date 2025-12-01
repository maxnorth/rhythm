# Testing Guide

## Overview

This guide covers testing practices for Rhythm, a multi-language durable execution framework with a Rust core and Python/Node.js adapters.

---

## Core Testing Principle: FFI Interfaces Drive Design

**The adapter interface (FFI) takes priority over test convenience.**

Tests must adapt to the interface required by language adapters (Python, Node.js), even if it results in less idiomatic or slightly verbose Rust test code.

### Why This Matters

Rhythm's architecture has three layers:
1. **Rust Core** - Performance-critical operations
2. **FFI Boundary** - Bridge between Rust and language adapters
3. **Language Adapters** - Python, Node.js, etc.

The FFI boundary is the **contract** between layers. Breaking it to make tests cleaner creates technical debt and confuses the architecture.

---

## FFI Testing Patterns

### ✅ Correct Pattern: Tests Adapt to Interface

```rust
// Keep the FFI interface as the primary function
pub async fn fail_execution(execution_id: &str, error: JsonValue, retry: bool) -> Result<()> {
    // ... implementation
}

// Tests use the real interface, even if slightly verbose
#[test]
async fn test_fail() {
    fail_execution(
        &id,
        serde_json::json!({"error": "Network error"}),
        false
    ).await.unwrap();
}
```

**Benefits:**
- ✅ FFI interface is clean and obvious
- ✅ Single source of truth for the function
- ✅ Tests validate the actual API consumers use
- ✅ No confusion about which function to call

### ❌ Anti-Pattern: Adapting Interface to Tests

```rust
// ❌ WRONG: Changed signature for test convenience
pub async fn fail_execution(execution_id: &str, error: &str, retry: bool) -> Result<()> {
    let error_json = serde_json::json!({"error": error});
    fail_execution_json(execution_id, error_json, retry).await
}

// Now need a separate function for the real interface
pub async fn fail_execution_json(execution_id: &str, error: JsonValue, retry: bool) -> Result<()> {
    // ... actual implementation
}
```

**Problems:**
- FFI now calls `fail_execution_json` instead of `fail_execution`
- The "real" function has an ugly name
- Creates confusion about which function is the canonical API
- Tests dictate production code design (tail wagging the dog)

### Guidelines

#### 1. FFI-First Design

When adding or modifying functions used across the FFI boundary:

1. **Start with the FFI requirements** - What do Python/Node.js need?
2. **Design the Rust signature** - Match FFI needs (e.g., `String`, `JsonValue`)
3. **Write tests using that signature** - Even if verbose

#### 2. When Test Convenience Conflicts with FFI

If a test would be cleaner with a different signature:

**Option A: Accept slightly verbose tests**
```rust
// Test uses the real interface
fail_execution(&id, serde_json::json!({"message": msg}), false).await
```

**Option B: Helper functions in tests (not production code)**
```rust
#[cfg(test)]
mod test_helpers {
    use super::*;

    pub async fn fail_execution_str(id: &str, error: &str, retry: bool) -> Result<()> {
        fail_execution(id, serde_json::json!({"message": error}), retry).await
    }
}

#[test]
async fn test_fail() {
    test_helpers::fail_execution_str(&id, "Network error", false).await.unwrap();
}
```

**Never do:**
- ❌ Change the public API signature for test convenience
- ❌ Create a `_json` variant when the main function should accept JSON
- ❌ Make the FFI call a secondary function

#### 3. Acceptable Test-Only Functions

Test helpers are fine when they:
- Live in `#[cfg(test)]` blocks or `tests.rs`
- Don't change the public API
- Provide convenience without altering contracts

```rust
#[cfg(test)]
mod test_utils {
    pub fn make_test_params(id: Option<String>) -> CreateExecutionParams {
        CreateExecutionParams {
            id,
            exec_type: ExecutionType::Task,
            target_name: "test.task".to_string(),
            queue: "test".to_string(),
            inputs: serde_json::json!({}),
            parent_workflow_id: None,
        }
    }
}
```

### Red Flags in Code Review

Watch for these patterns that suggest tests are driving interface design:

- ❌ Function suffixes like `_json`, `_raw`, `_internal` on the "real" implementation
- ❌ FFI calling anything other than the main public function
- ❌ Comments like "for tests" or "convenience wrapper" on public APIs
- ❌ Multiple functions doing the same thing with different signatures
- ❌ Tests using a simpler interface than FFI uses

---

## Prerequisites

1. **Rust** (latest stable)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   ```

2. **Python 3.8+** with pip

3. **Node.js 18+** with npm

4. **PostgreSQL 14+**
   ```bash
   make db
   ```

---

## Quick Start

### 1. Setup Database

```bash
make db          # Start PostgreSQL
make migrate     # Run migrations
```

### 2. Run Tests

**Rust Core:**
```bash
make core-test
```

**Python:**
```bash
cd python && pytest
```

**Node.js:**
```bash
cd node && npm test
```

---

## Manual Testing

### Step 1: Start PostgreSQL

```bash
make db
```

### Step 2: Run Migrations

```bash
make migrate
```

### Step 3: Enqueue Work (Python Example)

```bash
cd python
python examples/enqueue_example.py
```

Expected output:
```
============================================================
Enqueuing example tasks and workflows
============================================================

✓ Notification task enqueued: <execution-id>

✓ Order workflow enqueued: <workflow-id>

============================================================
Tasks enqueued! Start workers to process them:
  python -m rhythm worker -q notifications -q orders -m examples.simple_example
============================================================
```

### Step 4: Start Worker

In a separate terminal:

```bash
cd python
python -m rhythm worker -q notifications -q orders -m examples.simple_example
```

Expected output:
```
Imported module: examples.simple_example
Starting worker for queues: notifications, orders
2025-XX-XX XX:XX:XX [INFO] rhythm.worker: Worker <worker-id> initialized for queues: ['notifications', 'orders']
2025-XX-XX XX:XX:XX [INFO] rhythm.worker: Claimed task execution <execution-id>: send_notification
[NOTIFICATION] Sending to user user_123: Your order has been confirmed!
2025-XX-XX XX:XX:XX [INFO] rhythm.worker: Execution <execution-id> completed successfully
...
```

---

## Testing Components

### Unit Tests (Rust)

```bash
cd core
cargo test
```

The Rust tests cover:
- Execution creation and lifecycle
- Workflow suspension and resumption
- Database operations
- Error handling
- Idempotency (duplicate execution IDs)

### Unit Tests (Python)

```bash
cd python
pytest
```

### Unit Tests (Node.js)

```bash
cd node
npm test
```

---

## Integration Testing

### What Gets Tested

1. ✓ Rust core builds successfully
2. ✓ Database migrations run successfully
3. ✓ Python can import and use Rust extension
4. ✓ Node.js can use native module
5. ✓ Tasks can be enqueued
6. ✓ Workers can claim executions
7. ✓ Functions execute correctly
8. ✓ Results are stored
9. ✓ Workflows suspend and resume correctly
10. ✓ Child tasks are created and executed

### Running Integration Tests

Full end-to-end test:
```bash
# Start database
make db

# Run migration
make migrate

# Run Rust tests (includes DB integration)
make core-test
```

---

## Debugging

### Check Rust Build

```bash
cd core
cargo build --release
```

If this fails, check:
- Rust toolchain version: `rustc --version`
- Dependencies in `Cargo.toml`
- Compilation errors

### Check Database Connection

```bash
psql -h localhost -U rhythm -d rhythm -c "SELECT 1"
```

If this fails:
- Check `RHYTHM_DATABASE_URL` is set
- Verify PostgreSQL is running: `docker ps`
- Check database exists: `psql -h localhost -U rhythm -l`

### Enable Rust Logging

```bash
export RUST_LOG=debug
python -m rhythm worker ...
```

### Enable Python Logging

```bash
export PYTHONUNBUFFERED=1
python -m rhythm worker ...
```

---

## Common Issues

### Issue: Cannot connect to database

**Solution:** Start PostgreSQL and set the environment variable
```bash
make db
export RHYTHM_DATABASE_URL="postgresql://rhythm@localhost/rhythm"
```

### Issue: `Function 'xxx' not found in registry`

**Solution:** Import the module when starting the worker
```bash
python -m rhythm worker -q myqueue -m examples.simple_example
```

### Issue: Rust compilation errors

**Solution:** Update Rust and dependencies
```bash
rustup update
cd core
cargo update
cargo build
```

### Issue: Migration already applied

**Solution:** Reset the database
```bash
make db-reset
make migrate
```

---

## Performance Testing

### Benchmark Worker Throughput

```bash
# Terminal 1: Start worker
python -m rhythm worker -q bench -m examples.simple_example

# Terminal 2: Enqueue 1000 tasks
python -c "
import asyncio
from examples.simple_example import send_notification

async def main():
    for i in range(1000):
        await send_notification.queue(user_id=f'user_{i}', message='test')
    print('Enqueued 1000 tasks')

asyncio.run(main())
"
```

Monitor:
- Time to process all tasks
- Database CPU usage
- Worker memory usage

---

## Current Test Coverage

### What We Test

- ✅ Execution creation with custom IDs (idempotency)
- ✅ Execution lifecycle (pending → running → completed/failed)
- ✅ Workflow suspension and resumption
- ✅ Task retry logic (hardcoded 3 retries)
- ✅ Error handling and storage
- ✅ Database pool management
- ✅ Worker claiming logic
- ✅ Parent-child workflow relationships

### What We Don't Test (Removed Features)

- ❌ Priority queuing (removed)
- ❌ Heartbeats (removed)
- ❌ Signals (removed)
- ❌ Batch operations (removed)
- ❌ Per-execution retry configuration (removed, now hardcoded to 3)
- ❌ Timeout enforcement (removed)
- ❌ Worker tracking (removed)

---

## Summary

**Core Rule:** The FFI interface is the contract. Tests validate the contract, not convenience variants.

**When in doubt:**
1. What does the language adapter (Python/Node) need?
2. Design the Rust function for that use case
3. Tests use that same interface
4. If tests are verbose, that's okay - they're testing the real thing

**Remember:** A slightly verbose test that validates the real interface is better than a clean test that validates a fake interface.
