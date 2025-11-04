# Background Jobs Design

This document outlines potential strategies for running background jobs (like delay processing, cleanup tasks, etc.) in a distributed worker environment.

## Current Status

**Not yet implemented.** This is design documentation for future implementation.

## The Problem

Background jobs need to run periodically (e.g., every 1 second) to:
- Resume workflows with expired `Task.delay()` calls
- Clean up old completed executions
- Aggregate metrics
- Recover dead workers

**Challenge:** With multiple workers, how do we ensure:
1. Jobs run reliably (if one worker dies, another takes over)
2. Jobs don't run redundantly (avoid N workers doing the same work)
3. Minimal database load (avoid excessive queries)

## Approach 1: All Workers Participate (Simple but Wasteful)

### Design
Every worker runs the background job independently.

```rust
// Every worker does this
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        resume_expired_delays().await;
    }
});
```

### Behavior
- 10 workers = 10 identical UPDATE queries per second
- PostgreSQL handles race conditions naturally (first one wins)
- Others find 0 rows to update

### Pros
- Dead simple - no coordination needed
- No single point of failure
- Works if any worker is alive

### Cons
- Wasteful: N workers = N queries/second
- DB load scales linearly with worker count
- All workers compete for same rows

### When to Use
- Small number of workers (< 10)
- Query is fast and low-cost
- Simplicity > efficiency

---

## Approach 2: PostgreSQL Advisory Locks (Leader Election)

### Design
Use PostgreSQL advisory locks to elect a single leader.

```rust
async fn acquire_leadership(pool: &PgPool) -> Result<bool> {
    let acquired: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_lock(12345678)"  // Magic number for job type
    )
    .fetch_one(pool)
    .await?;

    Ok(acquired)
}

// In worker startup
if acquire_leadership(&pool).await? {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            resume_expired_delays().await;
        }
    });
}
```

### Behavior
- First worker to acquire lock becomes leader
- Lock automatically released when worker disconnects
- Another worker immediately acquires it (automatic failover)

### Pros
- Only 1 query/second (regardless of worker count)
- Automatic failover (lock released on disconnect)
- No external coordination service needed
- Built into PostgreSQL

### Cons
- All background work on one worker
- If leader is slow/stuck, delays until failover
- No visibility into which worker is leader

### When to Use
- Many workers (> 10)
- Background jobs are lightweight
- Want simplicity without external dependencies

---

## Approach 3: Work-Stealing with SKIP LOCKED

### Design
All workers try to grab batches of work, PostgreSQL prevents conflicts.

```rust
async fn resume_expired_delays_batch() -> Result<usize> {
    let resumed: Vec<String> = sqlx::query_scalar(
        r#"
        WITH expired AS (
            SELECT execution_id
            FROM workflow_execution_context wec
            JOIN executions delay ON delay.id = wec.awaiting_task_id
            WHERE delay.function_name = 'builtin.delay'
              AND (delay.checkpoint->>'resume_at')::TIMESTAMPTZ <= NOW()
            LIMIT 100
            FOR UPDATE SKIP LOCKED  -- Key: each worker gets different rows
        )
        UPDATE executions
        SET status = 'pending'
        WHERE id IN (SELECT execution_id FROM expired)
        RETURNING id
        "#
    )
    .fetch_all(pool.as_ref())
    .await?;

    Ok(resumed.len())
}

// All workers run this
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        let _ = resume_expired_delays_batch().await;
    }
});
```

### Behavior
- Each worker tries to grab a batch (LIMIT 100)
- `FOR UPDATE SKIP LOCKED` ensures no conflicts
- Work naturally distributes across workers
- If 1000 delays expired and 10 workers, each processes ~100

### Pros
- Simple: just run on all workers
- Work distributes automatically under load
- Fault tolerant: if worker dies, others pick up slack
- Scales: more work = more workers help

### Cons
- All workers query even when there's no work
- Slight overhead (N queries/second that may return 0 rows)

### When to Use
- Variable workload (sometimes heavy, sometimes light)
- Want automatic load distribution
- Have many workers

---

## Approach 4: Lease-Based Leader Election (Recommended)

### Design
Single-row table acts as a "lease". Workers compete to acquire it using `SKIP LOCKED`.

#### Schema
```sql
CREATE TABLE background_job_leases (
    job_name TEXT PRIMARY KEY,
    held_by_worker_id TEXT NOT NULL,
    lease_acquired_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB DEFAULT '{}'
);

INSERT INTO background_job_leases (job_name, held_by_worker_id)
VALUES ('delay_processor', 'none');
```

#### Implementation
```rust
pub struct LeaseGuard {
    job_name: String,
    worker_id: String,
    pool: Arc<PgPool>,
    heartbeat_task: Option<tokio::task::JoinHandle<()>>,
}

impl LeaseGuard {
    pub async fn try_acquire(
        job_name: &str,
        worker_id: &str,
        pool: Arc<PgPool>,
    ) -> Result<Option<Self>> {
        let acquired: Option<(String,)> = sqlx::query_as(
            r#"
            UPDATE background_job_leases
            SET held_by_worker_id = $1,
                lease_acquired_at = NOW(),
                heartbeat_at = NOW()
            WHERE job_name = $2
              AND (
                  held_by_worker_id = 'none'
                  OR heartbeat_at < NOW() - INTERVAL '10 seconds'
              )
            FOR UPDATE SKIP LOCKED
            RETURNING job_name
            "#
        )
        .bind(worker_id)
        .bind(job_name)
        .fetch_optional(pool.as_ref())
        .await?;

        if acquired.is_some() {
            // Start heartbeat every 2 seconds
            let heartbeat_task = tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(2));
                loop {
                    interval.tick().await;
                    sqlx::query(
                        "UPDATE background_job_leases
                         SET heartbeat_at = NOW()
                         WHERE job_name = $1 AND held_by_worker_id = $2"
                    )
                    .bind(job_name)
                    .bind(worker_id)
                    .execute(pool.as_ref())
                    .await;
                }
            });

            Ok(Some(LeaseGuard {
                job_name: job_name.to_string(),
                worker_id: worker_id.to_string(),
                pool,
                heartbeat_task: Some(heartbeat_task),
            }))
        } else {
            Ok(None)
        }
    }
}

impl Drop for LeaseGuard {
    fn drop(&mut self) {
        // Stop heartbeat
        if let Some(task) = self.heartbeat_task.take() {
            task.abort();
        }

        // Release lease
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                sqlx::query(
                    "UPDATE background_job_leases
                     SET held_by_worker_id = 'none'
                     WHERE job_name = $1 AND held_by_worker_id = $2"
                )
                .bind(&self.job_name)
                .bind(&self.worker_id)
                .execute(self.pool.as_ref())
                .await;
            });
        });
    }
}

// Worker loop
pub async fn start_delay_processor(worker_id: String, pool: Arc<PgPool>) {
    tokio::spawn(async move {
        let mut rng = rand::thread_rng();

        loop {
            match LeaseGuard::try_acquire("delay_processor", &worker_id, pool.clone()).await {
                Ok(Some(_guard)) => {
                    eprintln!("📋 Worker {} acquired lease for delay_processor", worker_id);

                    let mut interval = tokio::time::interval(Duration::from_secs(1));
                    loop {
                        interval.tick().await;
                        let _ = resume_expired_delays_batch(pool.clone()).await;
                    }
                    // Guard drops here, releases lease
                }
                Ok(None) => {
                    // Someone else has lease, backoff with jitter
                    let jitter = rng.gen_range(1000..5000);
                    tokio::time::sleep(Duration::from_millis(jitter)).await;
                }
                Err(e) => {
                    eprintln!("Error acquiring lease: {}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });
}
```

### Behavior
1. Workers compete for single lease row
2. First to acquire runs background job
3. Updates heartbeat every 2 seconds
4. If holder crashes (no heartbeat for 10s), next worker acquires
5. `SKIP LOCKED` ensures non-blocking checks
6. Jittered backoff prevents thundering herd

### Pros
- **Single active processor** - only one worker runs query at a time
- **No lock contention** - `SKIP LOCKED` prevents waiting
- **Automatic failover** - heartbeat-based detection (~10s)
- **Visible leadership** - query table to see who's leader
- **Graceful handoff** - Drop trait releases lease cleanly
- **Scalable** - ~3-5 queries/second total regardless of worker count
- **Multiple job types** - each job type gets its own lease row

### Cons
- More complex than simple approaches
- Requires heartbeat bookkeeping
- Failover latency (~10 seconds)

### Database Load
- Lease acquisition attempts: ~2-3 queries/second (with jitter across all workers)
- Heartbeat updates: 1 query per 2 seconds (only leader)
- Actual work: 1 query/second (only leader)
- **Total: ~3-5 queries/second regardless of worker count**

Compare to all-workers-participate: 10 workers × 1/sec = 10 queries/second

### When to Use
- **Production systems** with many workers
- Want minimal database load
- Need visibility into which worker is leader
- Background jobs should run reliably with automatic failover
- Multiple job types with independent leadership

### Monitoring
```sql
-- See current leaders
SELECT job_name, held_by_worker_id,
       NOW() - lease_acquired_at AS held_duration,
       NOW() - heartbeat_at AS last_heartbeat
FROM background_job_leases;

-- Alert if leader is stuck
SELECT * FROM background_job_leases
WHERE heartbeat_at > NOW() - INTERVAL '30 seconds'
  AND lease_acquired_at < NOW() - INTERVAL '5 minutes';
```

---

## Recommendation

For **production deployment**:
- Start with **Approach 4 (Lease-Based)** for reliability and efficiency
- Use separate lease rows for different job types (delay_processor, cleanup_job, etc.)
- Monitor lease table for leadership and health

For **development/testing**:
- Use **Approach 1 (All Workers)** for simplicity
- Easy to reason about, no coordination needed

For **high-scale** (1000+ workers):
- Consider **Approach 3 (Work-Stealing)** for natural load distribution
- Or use Approach 4 with periodic lease rotation (leader releases every 60s)

---

## Future Considerations

### Task.delay() Integration
Background jobs will be essential for:
- Resuming workflows with expired delays
- Completing "builtin.delay" executions
- Triggering parent workflow continuation

See continuation design docs for details on event-driven workflow resumption.

### Scheduled Jobs
If we add cron-like scheduled tasks, they can use the same lease infrastructure:
- Each scheduled job type gets a lease row
- Leader runs scheduled jobs on their schedule
- Same failover and visibility benefits

### Distributed Tracing
Add metadata to lease table:
- Last execution timestamp
- Items processed count
- Error count
- Performance metrics

This enables monitoring dashboard showing which workers are leaders and job health.
