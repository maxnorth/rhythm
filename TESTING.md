# Testing Guide

## Prerequisites

1. **Rust** (latest stable)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   ```

2. **Python 3.8+** with pip

3. **PostgreSQL 14+**
   ```bash
   docker-compose up -d
   ```

4. **Maturin** (for building Python extensions from Rust)
   ```bash
   pip install maturin
   ```

## Quick Start

### 1. Build the Project

```bash
./build.sh
```

This will:
- Build the Rust core with maturin
- Install the Python package in development mode
- Create the `workflows_core` Python extension module

### 2. Run Migrations

```bash
export WORKFLOWS_DATABASE_URL="postgresql://workflows:workflows@localhost/workflows"
python -m workflows migrate
```

### 3. Run End-to-End Test

```bash
./test_e2e.sh
```

## Manual Testing

### Step 1: Start PostgreSQL

```bash
docker-compose up -d
```

### Step 2: Build Rust Core

```bash
cd core
maturin develop --release
cd ..
```

### Step 3: Set Database URL

```bash
export WORKFLOWS_DATABASE_URL="postgresql://workflows:workflows@localhost/workflows"
```

### Step 4: Run Migrations

```bash
python -m workflows migrate
```

### Step 5: Enqueue Work

```bash
python examples/enqueue_example.py
```

Expected output:
```
============================================================
Enqueuing example jobs and workflows
============================================================

✓ Notification job enqueued: job_xxxxx

✓ Order workflow enqueued: wor_xxxxx

✓ High-priority order workflow enqueued: wor_xxxxx

============================================================
Jobs enqueued! Start workers to process them:
  currant worker -q notifications -q orders -m examples.simple_example
============================================================
```

### Step 6: Start Worker

In a separate terminal:

```bash
export WORKFLOWS_DATABASE_URL="postgresql://workflows:workflows@localhost/workflows"
python -m currant worker -q notifications -q orders -m examples.simple_example
```

Expected output:
```
Imported module: examples.simple_example
Starting worker for queues: notifications, orders
2025-10-05 XX:XX:XX [INFO] workflows.worker: Worker worker_xxxxx initialized for queues: ['notifications', 'orders']
2025-10-05 XX:XX:XX [INFO] workflows.worker: Worker worker_xxxxx starting...
2025-10-05 XX:XX:XX [INFO] workflows.worker: Worker worker_xxxxx listening on queues: ['notifications', 'orders']
2025-10-05 XX:XX:XX [INFO] workflows.worker: Claimed job execution job_xxxxx: examples.simple_example.send_notification
[NOTIFICATION] Sending to user user_123: Your order has been confirmed!
2025-10-05 XX:XX:XX [INFO] workflows.worker: Execution job_xxxxx completed successfully
...
```

## Testing Components

### Unit Tests (Rust)

```bash
cd core
cargo test
```

### Unit Tests (Python)

```bash
cd python
pytest
```

### Integration Tests

The E2E test covers:
1. ✓ Rust core builds successfully via maturin
2. ✓ Database migrations run via Rust
3. ✓ Python can import and use Rust extension (`workflows_core`)
4. ✓ Jobs can be enqueued via `RustBridge.create_execution()`
5. ✓ Worker can claim jobs via `RustBridge.claim_execution()`
6. ✓ Functions execute correctly
7. ✓ Results are stored via `RustBridge.complete_execution()`
8. ✓ Workflows suspend and resume correctly
9. ✓ Activities are created and executed
10. ✓ Worker heartbeats function via `RustBridge.update_heartbeat()`

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

### Check Python Extension

```bash
python -c "import workflows_core; print('OK')"
```

If this fails:
- Make sure maturin built successfully
- Check Python version compatibility (3.8+)
- Try rebuilding: `cd core && maturin develop --release`

### Check Database Connection

```bash
python -c "from workflows.rust_bridge import RustBridge; print('OK')"
```

If this fails:
- Check `WORKFLOWS_DATABASE_URL` is set
- Verify PostgreSQL is running: `docker ps`
- Test connection: `psql $WORKFLOWS_DATABASE_URL`

### Enable Rust Logging

```bash
export RUST_LOG=debug
python -m currant worker ...
```

### Enable Python Logging

```bash
export PYTHONUNBUFFERED=1
python -m currant worker ...
```

## Common Issues

### Issue: `ImportError: No module named 'workflows_core'`

**Solution:** Build the Rust extension
```bash
cd core
maturin develop
cd ..
```

### Issue: `cannot connect to database`

**Solution:** Start PostgreSQL
```bash
docker-compose up -d
export WORKFLOWS_DATABASE_URL="postgresql://workflows:workflows@localhost/workflows"
```

### Issue: `Function 'xxx' not found in registry`

**Solution:** Import the module when starting the worker
```bash
python -m currant worker -q myqueue -m examples.simple_example
```

### Issue: Rust compilation errors

**Solution:** Update Rust and dependencies
```bash
rustup update
cd core
cargo update
cargo build
```

## Performance Testing

### Benchmark Worker Throughput

```bash
# Terminal 1: Start worker
python -m currant worker -q bench -m examples.simple_example

# Terminal 2: Enqueue 1000 jobs
python -c "
import asyncio
from examples.simple_example import send_notification

async def main():
    for i in range(1000):
        await send_notification.queue(user_id=f'user_{i}', message='test')
    print('Enqueued 1000 jobs')

asyncio.run(main())
"
```

Monitor:
- Time to process all jobs
- Database CPU usage
- Worker memory usage

## Continuous Integration

Example GitHub Actions workflow:

```yaml
name: Test

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:16
        env:
          POSTGRES_USER: workflows
          POSTGRES_PASSWORD: workflows
          POSTGRES_DB: workflows
        ports:
          - 5432:5432

    steps:
      - uses: actions/checkout@v3

      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - uses: actions/setup-python@v4
        with:
          python-version: '3.11'

      - name: Install dependencies
        run: |
          pip install maturin
          ./build.sh

      - name: Run tests
        env:
          WORKFLOWS_DATABASE_URL: postgresql://workflows:workflows@localhost/workflows
        run: ./test_e2e.sh
```
