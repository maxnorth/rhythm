# Redis Backend Design Document

## Overview

This document outlines the design for adding optional Redis backend support to Currant, allowing users to choose between PostgreSQL's durability and Redis's performance on a per-task basis.

**Created:** 2025-10-11
**Status:** Design phase - not yet implemented
**Goal:** Enable high-throughput use cases while maintaining PostgreSQL as the default for durability

---

## Performance Context

### Current Performance (PostgreSQL only)
- **Throughput:** ~200 tasks/sec sustained with 5 workers
- **Latency:** p50=5.2ms, p95=9.4ms, p99=16.7ms
- **Durability:** Full ACID guarantees, survives crashes
- **Optimization journey:** Started at 24 tasks/sec, achieved 8.3x improvement through:
  - LISTEN/NOTIFY for instant notifications
  - Batch claiming (multiple tasks per query)
  - Global connection pool (fixed major bottleneck)
  - Local task queue with prefetching
  - Batch completion (1ms flush interval)

### Competitive Landscape

**Redis-backed queues (Dramatiq, Celery, Huey):**
- Throughput: 1,700-5,500 tasks/sec (10 workers)
- Trade-off: In-memory storage, can lose tasks on crash
- Requires separate Redis service

**PostgreSQL-backed durable queues:**
- **Temporal:** 8-16 workflows/sec (complex workflow engine)
- **Currant:** 200 tasks/sec (10-20x faster than Temporal!)
- **pg_boss:** 100-500 tasks/sec
- **Graphile Worker:** 200-1,000 tasks/sec

### Target Performance with Redis
- **Redis + eventual durability:** 800-1,500 tasks/sec (~4-7x improvement)
- **Redis only (no durability):** 2,000-4,000 tasks/sec (~10-20x improvement)
- **Hybrid mode:** Best of both worlds for different task types

---

## Design Philosophy

### Core Principle: Per-Task Backend Selection

Users should be able to choose the appropriate backend for each task type based on their specific needs:

```python
# Critical financial transaction - full durability
@task(queue="payments", backend="postgres")
async def process_payment(order_id: int):
    await payment_gateway.charge(order_id)

# High-volume analytics - eventual durability acceptable
@task(queue="analytics", backend="redis", durability="eventual")
async def track_event(event: dict):
    await analytics.track(event)

# Ultra high-throughput metrics - ephemeral is fine
@task(queue="metrics", backend="redis", durability="none")
async def increment_counter(metric: str):
    await metrics.increment(metric)
```

### Design Decisions

1. **PostgreSQL remains the default** - Backward compatible, safe by default
2. **Redis is optional** - Users opt-in when they need performance
3. **Per-task granularity** - Different tasks can use different backends
4. **Transparent to workers** - Workers don't care which backend is used
5. **No code changes to task functions** - Just decorator configuration

---

## Architecture

### Backend Abstraction Layer

```rust
// core/src/queue_backend.rs
#[async_trait]
pub trait QueueBackend: Send + Sync {
    /// Enqueue a task ID to the queue
    async fn enqueue(&self, execution_id: &str, queue: &str, priority: i32) -> Result<()>;

    /// Claim multiple task IDs from the queue
    async fn claim(&self, worker_id: &str, queues: &[String], limit: i32) -> Result<Vec<String>>;

    /// Notify workers that a task is available
    async fn notify(&self, queue: &str, execution_id: &str) -> Result<()>;

    /// Remove a task from the queue (after completion/failure)
    async fn remove(&self, execution_id: &str, queue: &str) -> Result<()>;

    /// Backend name for logging/debugging
    fn name(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct BackendConfig {
    pub backend: String,      // "postgres", "redis", "hybrid"
    pub durability: String,   // "full", "eventual", "none"
}
```

### Three Backend Implementations

#### 1. PostgresBackend (Current Implementation)
```
Enqueue → PostgreSQL INSERT
Claim   → PostgreSQL UPDATE ... FOR UPDATE SKIP LOCKED
Notify  → PostgreSQL NOTIFY
```
- **Pros:** ACID guarantees, no external dependencies
- **Cons:** ~200 tasks/sec limit
- **Use case:** Payments, critical data, default choice

#### 2. RedisBackend (High Performance)
```
Enqueue → Redis ZADD (sorted set by priority/timestamp)
       → Redis PUBLISH (notify workers)
       → [Optional] PostgreSQL INSERT (async, for durability="eventual")

Claim   → Redis ZPOPMIN (atomic pop from sorted set)

Notify  → Redis PUBLISH/SUBSCRIBE

Complete → Redis ZREM
        → [Optional] PostgreSQL UPDATE (for durability="eventual")
```
- **Pros:** 2,000-4,000+ tasks/sec throughput
- **Cons:** Can lose tasks on Redis crash (durability="none")
- **Use case:** Analytics, metrics, high-volume non-critical tasks

#### 3. HybridBackend (Optional Future Enhancement)
```
Enqueue → Redis ZADD (fast claiming)
       → PostgreSQL INSERT (immediate durability)

Claim   → Try Redis ZPOPMIN first (fast path)
       → Fallback to PostgreSQL (if Redis empty/failed)
       → Repopulate Redis from PostgreSQL (cache warming)
```
- **Pros:** Fast claiming with PostgreSQL durability
- **Cons:** More complex, two systems to manage
- **Use case:** High throughput with safety net

---

## Python API Design

### Task Decorator Enhancement

```python
from currant import task

@task(
    queue: str,
    backend: str = "postgres",        # "postgres" | "redis" | "hybrid"
    durability: str = "full",         # "full" | "eventual" | "none"
    retries: int = None,
    timeout: int = None,
    priority: int = 5,
)
```

### Durability Modes

| Mode | Backend | PostgreSQL | Redis | Survives Redis Crash | Throughput |
|------|---------|------------|-------|---------------------|------------|
| `full` | `postgres` | ✓ Immediate | ✗ | ✓ | ~200 tasks/sec |
| `eventual` | `redis` | ✓ Async write | ✓ Queue | ✓ (eventually) | ~800-1,500 tasks/sec |
| `none` | `redis` | ✗ | ✓ Queue only | ✗ | ~2,000-4,000+ tasks/sec |

### Configuration

```python
# Environment variables
CURRANT_DATABASE_URL="postgresql://..."
CURRANT_REDIS_URL="redis://localhost:6379"  # Optional

# Or in config
from currant.config import settings
settings.redis_url = "redis://localhost:6379"
```

---

## Implementation Phases

### Phase 1: Foundation (Week 1)
**Goal:** Basic Redis pub/sub for notifications only

- [ ] Add Redis dependency to Cargo.toml
- [ ] Create `QueueBackend` trait
- [ ] Implement `PostgresBackend` (wraps existing logic)
- [ ] Add Redis pub/sub for NOTIFY replacement
- [ ] Keep PostgreSQL for all queue operations
- [ ] Test notification performance improvement

**Expected gain:** 20-30% improvement from faster notifications
**Risk:** Low - PostgreSQL still handles all queue operations

### Phase 2: Redis Queue (Week 2-3)
**Goal:** Full Redis backend with durability options

- [ ] Implement `RedisBackend` with sorted sets
- [ ] Add `backend` parameter to `@task()` decorator
- [ ] Update `create_execution()` to route based on backend
- [ ] Implement durability modes:
  - [ ] `durability="none"` - Redis only
  - [ ] `durability="eventual"` - Redis + async PostgreSQL
- [ ] Update workers to claim from Redis
- [ ] Add Redis connection pooling
- [ ] Benchmark Redis performance

**Expected gain:** 10-20x improvement for Redis tasks
**Risk:** Medium - new code path, need thorough testing

### Phase 3: Hybrid Mode (Week 4+)
**Goal:** Redis cache with PostgreSQL fallback (optional)

- [ ] Implement `HybridBackend`
- [ ] Cache warming from PostgreSQL to Redis
- [ ] Fallback logic when Redis unavailable
- [ ] Performance tuning

**Expected gain:** Best of both worlds
**Risk:** High - complex coordination between systems

---

## Data Flow Examples

### Example 1: durability="full" (PostgreSQL)
```
1. User calls: send_email.queue(to="user@example.com")
2. create_execution() writes to PostgreSQL executions table
3. PostgreSQL NOTIFY sent to queue channel
4. Worker receives NOTIFY via asyncpg
5. Worker claims via batch UPDATE ... FOR UPDATE SKIP LOCKED
6. Worker executes, completes via batch completion
7. PostgreSQL UPDATE executions SET status='completed'
```

### Example 2: durability="none" (Redis Only)
```
1. User calls: track_event.queue(event={...})
2. create_execution() skips PostgreSQL (or minimal metadata only)
3. Redis ZADD to currant:queue:analytics:p5
4. Redis PUBLISH to currant:notify:analytics
5. Worker subscribed to currant:notify:analytics receives message
6. Worker claims via Redis ZPOPMIN (atomic)
7. Worker executes, completes via Redis ZREM
8. No PostgreSQL writes (except maybe final result for history)
```

### Example 3: durability="eventual" (Redis + Async PostgreSQL)
```
1. User calls: generate_thumbnail.queue(image_id=123)
2. Redis ZADD (immediate, fast response)
3. Redis PUBLISH notification
4. Background task: PostgreSQL INSERT (async, non-blocking)
5. Worker claims via Redis ZPOPMIN
6. Worker executes
7. Worker completes: Redis ZREM + PostgreSQL UPDATE (async)
8. If Redis crashes: tasks can be recovered from PostgreSQL
```

---

## Redis Data Structures

### Queue Storage (Sorted Sets)
```
Key: currant:queue:{queue_name}:p{priority}
Score: timestamp (FIFO within priority)
Members: execution_id

Example:
currant:queue:emails:p5 → {
  "job_abc123": 1696800000.123,
  "job_def456": 1696800000.456,
}

Commands:
- Enqueue: ZADD currant:queue:emails:p5 {timestamp} job_abc123
- Claim: ZPOPMIN currant:queue:emails:p5 10  (claim 10 tasks)
- Check size: ZCARD currant:queue:emails:p5
```

### Notification (Pub/Sub)
```
Channel: currant:notify:{queue_name}
Message: execution_id

Commands:
- Notify: PUBLISH currant:notify:emails job_abc123
- Listen: SUBSCRIBE currant:notify:emails
```

### Priority Handling
- Separate sorted sets per priority level (0-10)
- Workers check highest priority first: p10, p9, p8, ..., p0
- Round-robin across queues with same priority

---

## Worker Changes

### Current Worker Architecture
```python
class Worker:
    async def _claimer_loop(self):
        # Claim from PostgreSQL via batch query
        claimed = RustBridge.claim_executions_batch(...)

    async def _listener_loop(self):
        # Listen for PostgreSQL NOTIFY
        await self.notify_event.wait()
```

### Enhanced Worker (Backend-Aware)
```python
class Worker:
    def __init__(self, queues: list[str], backends: list[str] = ["postgres"]):
        self.backends = backends  # ["postgres", "redis"] or ["postgres"]
        self.redis_conn = None if "redis" not in backends else self._setup_redis()

    async def _claimer_loop(self):
        # Try Redis first if enabled
        if "redis" in self.backends and self.redis_conn:
            claimed_redis = await self._claim_from_redis(...)
            if claimed_redis:
                return claimed_redis

        # Fallback or primary: PostgreSQL
        claimed_pg = await self._claim_from_postgres(...)
        return claimed_pg

    async def _listener_loop(self):
        if "redis" in self.backends:
            # Redis pub/sub (faster)
            await self._listen_redis_pubsub()
        else:
            # PostgreSQL NOTIFY (current)
            await self._listen_postgres_notify()
```

---

## Configuration & Environment

### Environment Variables
```bash
# Required
CURRANT_DATABASE_URL="postgresql://user:pass@localhost/currant"

# Optional (enables Redis backend)
CURRANT_REDIS_URL="redis://localhost:6379"
CURRANT_REDIS_PASSWORD="secret"
CURRANT_REDIS_DB="0"

# Redis connection pool settings
CURRANT_REDIS_MAX_CONNECTIONS="50"
CURRANT_REDIS_MIN_CONNECTIONS="5"
```

### Runtime Configuration
```python
from currant.config import Settings

settings = Settings(
    redis_url="redis://localhost:6379",
    redis_enabled=True,
    redis_max_connections=50,
)
```

---

## Testing Strategy

### Unit Tests
- [ ] Backend trait implementations
- [ ] Redis connection handling
- [ ] Durability mode logic
- [ ] Fallback mechanisms

### Integration Tests
- [ ] End-to-end task execution with Redis backend
- [ ] PostgreSQL fallback when Redis unavailable
- [ ] Mixed backend workloads (some tasks Redis, some PostgreSQL)
- [ ] Redis crash recovery (durability="eventual")

### Benchmarks
- [ ] Redis vs PostgreSQL throughput comparison
- [ ] Latency under different load levels
- [ ] Durability mode performance impact
- [ ] Multi-worker scaling

### Benchmark Scenarios
```python
# Scenario 1: Pure PostgreSQL (baseline)
@task(queue="test", backend="postgres")
async def noop_pg(): pass

# Scenario 2: Redis no durability (max speed)
@task(queue="test", backend="redis", durability="none")
async def noop_redis_fast(): pass

# Scenario 3: Redis eventual durability (balanced)
@task(queue="test", backend="redis", durability="eventual")
async def noop_redis_eventual(): pass

# Run benchmarks
currant bench --workers 5 --tasks 1000 --backend postgres
currant bench --workers 5 --tasks 1000 --backend redis --durability none
currant bench --workers 5 --tasks 1000 --backend redis --durability eventual
```

---

## Migration Strategy

### Backward Compatibility
- All existing code continues to work without changes
- PostgreSQL remains the default backend
- No breaking changes to API

### Opt-In Process
1. **Install Redis** (optional): `docker run -p 6379:6379 redis`
2. **Set environment variable**: `CURRANT_REDIS_URL="redis://localhost:6379"`
3. **Update tasks selectively**: Add `backend="redis"` to high-throughput tasks
4. **No worker changes needed**: Workers auto-detect enabled backends

### Rollback Strategy
- Remove `backend="redis"` from task decorators
- Tasks revert to PostgreSQL automatically
- No data loss (with durability="eventual")

---

## Operational Considerations

### Monitoring
- [ ] Track backend usage per queue
- [ ] Monitor Redis connection pool health
- [ ] Alert on Redis unavailability
- [ ] Dashboard for PostgreSQL vs Redis throughput

### Redis High Availability
- Redis Sentinel for automatic failover
- Redis Cluster for horizontal scaling
- AOF persistence for durability="eventual" tasks

### Capacity Planning
| Workers | Backend | Expected Throughput | Redis Memory (1M tasks) |
|---------|---------|---------------------|------------------------|
| 5 | PostgreSQL | ~200 tasks/sec | - |
| 5 | Redis (none) | ~2,000 tasks/sec | ~50 MB |
| 5 | Redis (eventual) | ~1,000 tasks/sec | ~50 MB |
| 10 | Redis (none) | ~4,000 tasks/sec | ~50 MB |

### Failure Scenarios

**Redis crash (durability="none"):**
- In-flight tasks lost
- Workers fall back to PostgreSQL
- Resume when Redis available

**Redis crash (durability="eventual"):**
- Tasks being written to PostgreSQL in background
- Some recent tasks may be lost (< 1 second window)
- Workers can recover from PostgreSQL
- Resume Redis when available

**PostgreSQL crash (all modes):**
- Critical: System halts (durability="full")
- Degraded: Redis continues, durability lost (durability="eventual")
- Need PostgreSQL for execution metadata and results

---

## Security Considerations

### Redis Authentication
- Require `CURRANT_REDIS_PASSWORD` in production
- Use Redis ACLs to limit command access
- TLS encryption for Redis connections

### Data Isolation
- Use Redis key prefixes: `currant:queue:{queue_name}`
- Separate Redis DB per environment (dev/staging/prod)
- Avoid mixing Currant and other Redis data

---

## Performance Expectations

### Projected Throughput

| Configuration | Throughput | Latency (p50) | Durability |
|--------------|-----------|---------------|------------|
| PostgreSQL (current) | 200 tasks/sec | 5.2ms | Full ACID |
| Redis + eventual | 800-1,500 tasks/sec | 2-3ms | Async persist |
| Redis only | 2,000-4,000 tasks/sec | 1-2ms | None |

### Scaling Characteristics
- **PostgreSQL:** Linear to ~10 workers, then DB becomes bottleneck
- **Redis:** Linear to ~20 workers, then Redis CPU/network bottleneck
- **Hybrid:** Best case - Redis speed with PostgreSQL safety

---

## Decision Log

### Why per-task backend selection?
- Different tasks have different requirements
- Critical tasks need durability, high-volume tasks need speed
- User knows their data better than the framework
- Allows gradual migration and experimentation

### Why not use Redis for everything?
- PostgreSQL provides valuable ACID guarantees
- Many use cases don't need extreme throughput
- Redis is another operational dependency
- Safe defaults are better (PostgreSQL)

### Why three durability modes?
- `full`: For users who need guarantees (default)
- `eventual`: For users who want speed but need recoverability
- `none`: For users who need maximum speed and can tolerate loss

### Why not implement Redis first?
- PostgreSQL performance is already competitive (8.3x improvement achieved)
- Validate the abstraction layer design first
- Ensure backward compatibility is solid
- Redis can be added incrementally without disrupting existing users

---

## Future Enhancements

### Phase 4+: Additional Backends (Future)
- Amazon SQS backend
- RabbitMQ backend
- In-memory backend (testing)
- Custom backend plugins

### Advanced Features
- Automatic backend selection based on queue stats
- Dynamic backend switching (hot-swap)
- Multi-region Redis replication
- Backend performance auto-tuning

---

## References

### Benchmarks Referenced
- Celery with Redis: 1,700 tasks/sec (10 workers, threads)
- Dramatiq with Redis: 4,800 tasks/sec (10 workers, threads)
- Temporal with PostgreSQL: 8-16 workflows/sec (4 vCores)
- Currant with PostgreSQL: 200 tasks/sec (5 workers) ← **Current state**

### Related Reading
- [Redis Sorted Sets Documentation](https://redis.io/docs/data-types/sorted-sets/)
- [Redis Pub/Sub Documentation](https://redis.io/docs/manual/pubsub/)
- [PostgreSQL LISTEN/NOTIFY Documentation](https://www.postgresql.org/docs/current/sql-notify.html)
- [Dramatiq Architecture](https://dramatiq.io/architecture.html)

---

## Conclusion

The Redis backend design provides a clear path to 10-20x performance improvement for high-throughput use cases while maintaining PostgreSQL as the reliable default. The per-task backend selection gives users fine-grained control over the speed/durability tradeoff, making Currant suitable for both critical financial transactions and high-volume analytics workloads.

**Implementation Status:** Design complete, awaiting implementation
**Next Step:** Phase 1 - Redis pub/sub notifications
**Target:** Q1 2025 (if prioritized)
