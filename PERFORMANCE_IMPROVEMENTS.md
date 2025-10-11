# Currant Performance Improvements

## Goal
Transform Currant into a professional-grade async scheduler that can compete with Celery, Temporal, and BullMQ.

**Target Performance:**
- **Throughput**: 10,000+ jobs/sec with 50 workers
- **Latency**: p99 < 50ms for noop jobs
- **Efficiency**: Near-zero DB load when idle
- **Scalability**: Linear scaling with worker count

## Current Issues (Identified)

### 1. Aggressive Polling (CRITICAL)
- `_listener_loop`: Sleeps 0.1s then tries to claim → 10 queries/sec per worker when idle
- `_poll_loop`: Additional query every 1s as "fallback"
- **Impact**: 100 idle workers = 1000+ unnecessary queries/sec
- **Fix**: Implement proper LISTEN/NOTIFY

### 2. No Real LISTEN/NOTIFY Implementation
- Code sends NOTIFY but workers don't actually LISTEN
- The "listener_loop" is just polling, not listening
- **Fix**: Add asyncpg connection for LISTEN, wake workers on NOTIFY

### 3. Single-Job Claiming
- Worker claims 1 job at a time via DB query
- High DB round-trip overhead
- **Fix**: Batch-claim up to `max_concurrent` jobs in one query

### 4. No Prefetching
- Worker waits for capacity, then claims
- Leads to idle time between job completions
- **Fix**: Prefetch jobs into local queue when below 50% capacity

### 5. Inefficient Execution Model
- Single claim attempt per loop iteration
- Blocking claim operation
- **Fix**: Parallel claim attempts, async task pool

## Implementation Plan

### Phase 1: LISTEN/NOTIFY (Eliminate Polling)
**Files**: `core/src/executions.rs`, `python/currant/worker.py`

1. Add separate asyncpg connection for LISTEN in Python worker
2. Replace `_listener_loop` polling with blocking wait on NOTIFY
3. Keep `_poll_loop` as fallback only (5s interval instead of 1s)
4. Measure: DB query rate when idle should drop to ~0.2/sec (heartbeat only)

### Phase 2: Batch Claiming
**Files**: `core/src/executions.rs`, `core/src/lib.rs`, `python/currant/worker.py`

1. Add `claim_executions_batch(worker_id, queues, limit)` function
2. Modify SQL: `LIMIT 1` → `LIMIT $3`
3. Return `Vec<Execution>` instead of `Option<Execution>`
4. Update Python worker to claim batch when capacity available
5. Measure: Throughput should increase 3-5x

### Phase 3: Local Job Queue + Prefetching
**Files**: `python/currant/worker.py`

1. Add `asyncio.Queue` as local job buffer (max size = `max_concurrent * 2`)
2. Separate "claimer" task that keeps queue filled
3. Execution tasks pull from local queue
4. Prefetch when queue < 50% full
5. Measure: Worker should maintain 100% utilization under load

### Phase 4: Concurrent Execution Optimization
**Files**: `python/currant/worker.py`

1. Use `asyncio.Semaphore` for concurrency control (cleaner than counter)
2. Fire-and-forget execution tasks
3. Separate task pools for claiming vs executing
4. Measure: Reduced latency variance

### Phase 5: Connection Pool Tuning
**Files**: `core/src/db.rs`, `python/currant/config.py`

1. Increase default pool size based on `max_concurrent`
2. Add idle connection timeout
3. Add connection health checks
4. Measure: Reduced connection errors under load

### Phase 6: Benchmark Improvements
**Files**: `core/src/benchmark.rs`

1. Add `--rate` limiting (spread job enqueueing over time)
2. Add real-time throughput monitoring during test
3. Add latency histogram output
4. Add DB query count measurement
5. Separate "system capacity" vs "steady state" tests

## Validation Strategy

Each phase will be validated with:
```bash
# Baseline (before changes)
python -m currant bench --workers 10 --jobs 1000 --warmup-percent 10

# After each phase
python -m currant bench --workers 10 --jobs 1000 --warmup-percent 10
python -m currant bench --workers 50 --jobs 10000 --warmup-percent 10

# Stress test
python -m currant bench --workers 100 --jobs 50000 --duration 60s
```

## Success Metrics

| Metric | Baseline (Expected) | Target |
|--------|---------------------|--------|
| Throughput (10 workers, 1000 jobs) | ~100-200/sec | 500+/sec |
| Throughput (50 workers, 10000 jobs) | ~500/sec | 5000+/sec |
| p99 Latency (noop job) | 200-500ms | <50ms |
| DB queries when idle (100 workers) | 1000+/sec | <20/sec |

## Non-Goals (Scope Control)

- HTTP API improvements (out of scope)
- Schema migrations (keep compatible)
- New features (performance only)
- Multi-region support (future work)
