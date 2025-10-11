# Benchmark Improvements TODO

This document tracks planned improvements to the benchmark implementation in `core/src/benchmark.rs`.

## Context

After a critical review of the benchmark implementation, we identified 8 high-value improvements to make it "professional grade". These were validated against the actual codebase to ensure they're feasible and valuable.

**Files to modify:**
- `core/src/benchmark.rs` - Main benchmark implementation
- `core/src/cli.rs` - Add new CLI flags for benchmark command

**Testing approach:**
After each change, test with:
```bash
cd python
.venv/bin/python -m currant bench --workers 2 --jobs 100
```

---

## 1. Fix Non-Deterministic Queue Distribution Algorithm

**Status:** Not started
**Priority:** CRITICAL (breaks functionality)
**Complexity:** Low (20 lines)

### Problem

The `select_queue()` function at line 303-316 has a broken distribution algorithm:

```rust
let position = (index as f64 / 1000.0) % 1.0;
```

This divides by a magic number 1000, causing:
- With 500 jobs and 50/50 distribution: ALL jobs go to first queue
- With 1000 jobs: Works correctly (by accident)
- With 1500 jobs: First 500 go to q1, next 500 to q2, last 500 back to q1

**Validated with test:** Created test_queue_dist.rs that confirmed with 500 jobs and 50/50 split, q1 gets 100% of jobs.

### Solution

Replace with proper round-robin or weighted distribution:

```rust
fn select_queue<'a>(queues: &[&'a str], distribution: &[f64], index: usize) -> &'a str {
    let mut cumulative = 0.0;
    // Use index directly, normalize to 0-1 range properly
    let total_items = 1000; // Or pass as parameter
    let position = (index % total_items) as f64 / total_items as f64;

    for (i, &percentage) in distribution.iter().enumerate() {
        cumulative += percentage;
        if position < cumulative {
            return queues[i];
        }
    }

    queues[queues.len() - 1]
}
```

**Better approach:** Use deterministic round-robin:
```rust
fn select_queue<'a>(queues: &[&'a str], distribution: &[f64], index: usize) -> &'a str {
    // Build cumulative buckets: [0.5, 1.0] for 50/50 split
    // For each index, map to bucket deterministically

    // Simple approach: create virtual queue array matching distribution
    // e.g., for [0.5, 0.5] with 1000 jobs: [q1]*500 + [q2]*500
    // Then select by index % total
}
```

**Testing:**
- Test with 500 jobs, 50/50 → should get ~250/250
- Test with 1000 jobs, 30/70 → should get ~300/700
- Test with 2000 jobs, 33/33/34 (3 queues) → should get ~660/660/680

### Files to change
- `core/src/benchmark.rs` line 303-316

---

## 2. Add Percentile Latency Metrics (p50, p95, p99)

**Status:** Not started
**Priority:** HIGH (professional standard)
**Complexity:** Medium (40 lines)

### Problem

Currently only collecting AVG latency (line 378):
```rust
CAST(AVG(EXTRACT(EPOCH FROM (completed_at - created_at)) * 1000) AS DOUBLE PRECISION) as avg_duration_ms
```

Average doesn't show tail latency, which is critical for SLA validation.

### Solution

PostgreSQL supports `percentile_cont()` function (validated: works on postgres:16-alpine).

**Update BenchmarkMetrics struct** (around line 26):
```rust
struct BenchmarkMetrics {
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    enqueued_jobs: usize,
    enqueued_workflows: usize,
    completed_jobs: i64,
    failed_jobs: i64,
    pending_jobs: i64,
    avg_latency_ms: f64,
    p50_latency_ms: f64,  // NEW
    p95_latency_ms: f64,  // NEW
    p99_latency_ms: f64,  // NEW
}
```

**Update collect_metrics query** (line 372-386):
```rust
let row: (i64, i64, i64, Option<f64>, Option<f64>, Option<f64>, Option<f64>) = sqlx::query_as(
    r#"
    WITH latencies AS (
        SELECT EXTRACT(EPOCH FROM (completed_at - created_at)) * 1000 as latency_ms
        FROM executions
        WHERE created_at >= $1 AND created_at <= $2
          AND status = 'completed'
    )
    SELECT
        (SELECT COUNT(*) FROM executions WHERE created_at >= $1 AND created_at <= $2 AND status = 'completed') as completed,
        (SELECT COUNT(*) FROM executions WHERE created_at >= $1 AND created_at <= $2 AND status = 'failed') as failed,
        (SELECT COUNT(*) FROM executions WHERE created_at >= $1 AND created_at <= $2 AND status = 'pending') as pending,
        (SELECT AVG(latency_ms) FROM latencies) as avg_latency,
        (SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY latency_ms) FROM latencies) as p50,
        (SELECT percentile_cont(0.95) WITHIN GROUP (ORDER BY latency_ms) FROM latencies) as p95,
        (SELECT percentile_cont(0.99) WITHIN GROUP (ORDER BY latency_ms) FROM latencies) as p99
    "#,
)
```

**Update display_report** (line 435-460):
```rust
println!("📈 Latency:");
println!("   Average: {:.1}ms", metrics.avg_latency_ms);
println!("   p50: {:.1}ms", metrics.p50_latency_ms);
println!("   p95: {:.1}ms", metrics.p95_latency_ms);
println!("   p99: {:.1}ms", metrics.p99_latency_ms);
```

### Files to change
- `core/src/benchmark.rs` lines 26-35 (struct), 362-398 (collect_metrics), 435-460 (display_report)

---

## 3. Separate Job vs Workflow Latency Metrics

**Status:** Not started
**Priority:** HIGH (better visibility)
**Complexity:** Medium (50 lines)

### Problem

Current metrics mix jobs and workflows together. Workflows inherently have higher latency (they coordinate activities), so mixing them skews the average.

### Solution

The `executions` table has a `type` column with values: 'job', 'activity', 'workflow' (validated via `\d executions`).

**Approach 1: Separate metrics structs**
```rust
struct LatencyMetrics {
    count: i64,
    avg_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
}

struct BenchmarkMetrics {
    // ... existing fields ...
    job_latency: LatencyMetrics,
    workflow_latency: LatencyMetrics,
}
```

**Update query to GROUP BY type:**
```sql
WITH latencies AS (
    SELECT
        type,
        EXTRACT(EPOCH FROM (completed_at - created_at)) * 1000 as latency_ms
    FROM executions
    WHERE created_at >= $1 AND created_at <= $2
      AND status = 'completed'
)
SELECT
    type,
    COUNT(*) as count,
    AVG(latency_ms) as avg,
    percentile_cont(0.5) WITHIN GROUP (ORDER BY latency_ms) as p50,
    percentile_cont(0.95) WITHIN GROUP (ORDER BY latency_ms) as p95,
    percentile_cont(0.99) WITHIN GROUP (ORDER BY latency_ms) as p99
FROM latencies
GROUP BY type
```

Then fetch multiple rows and populate job_latency vs workflow_latency.

**Display:**
```
📈 Job Latency:
   Count: 1000
   Average: 45.2ms
   p50: 42.1ms | p95: 78.3ms | p99: 125.6ms

📈 Workflow Latency:
   Count: 100
   Average: 234.5ms
   p50: 201.3ms | p95: 456.7ms | p99: 678.9ms
```

### Files to change
- `core/src/benchmark.rs` lines 26-35 (structs), 362-398 (query), 435-460 (display)

---

## 4. Add Cleanup Guard to Ensure Workers Stop on Failure

**Status:** Not started
**Priority:** HIGH (prevents resource leaks)
**Complexity:** Low (15 lines)

### Problem

If `enqueue_jobs()` (line 79), `enqueue_workflows()` (line 80), or `wait_for_completion()` (line 99) fails, workers keep running as orphaned processes.

Example failure scenario:
1. Spawn 10 workers (line 65)
2. Database connection fails during enqueueing (line 79)
3. Function returns error
4. Workers never stopped → orphaned processes

### Solution

Create a RAII guard that stops workers on drop:

```rust
struct WorkerGuard {
    workers: Vec<Child>,
}

impl WorkerGuard {
    fn new(workers: Vec<Child>) -> Self {
        Self { workers }
    }

    fn into_inner(self) -> Vec<Child> {
        // Prevent drop by consuming self
        let workers = std::mem::take(&mut self.workers);
        std::mem::forget(self);
        workers
    }
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        if !self.workers.is_empty() {
            eprintln!("⚠️  Cleaning up {} workers due to early exit", self.workers.len());
            let workers = std::mem::take(&mut self.workers);
            let _ = stop_workers(workers);
        }
    }
}
```

**Usage in run_benchmark:**
```rust
let workers = spawn_workers(params.workers, &params.queues)?;
let worker_guard = WorkerGuard::new(workers);

// ... do work ...

// Success path: extract workers and stop normally
let workers = worker_guard.into_inner();
stop_workers(workers)?;
```

**Alternative simpler approach** using scopeguard crate (if we add dependency):
```rust
use scopeguard::defer;

let workers = spawn_workers(params.workers, &params.queues)?;
defer! {
    let _ = stop_workers(workers);
}
```

### Files to change
- `core/src/benchmark.rs` - Add struct before run_benchmark, modify run_benchmark to use guard
- `core/Cargo.toml` - Optional: add scopeguard = "1.2" dependency

### Testing
- Modify code to fail after spawning workers (e.g., `return Err(anyhow!("test"))`)
- Run benchmark, verify workers are killed
- Check `ps aux | grep currant` shows no orphaned workers

---

## 5. Add Warmup Period to Exclude Cold-Start Executions

**Status:** Not started
**Priority:** MEDIUM (accurate measurements)
**Complexity:** Medium (30 lines)

### Problem

When you enqueue 1000 jobs instantly, the first ~50 jobs might experience:
- Cold connection pool (first connections being established)
- Low contention (workers haven't ramped up yet)
- Different DB query plans (first execution)

This skews latency metrics toward optimistic values.

**Example scenario:**
- Enqueue 1000 jobs in 1 second
- First 50 complete in 10ms each (unrealistic, low load)
- Jobs 51-1000 complete in 50ms each (realistic steady state)
- Average includes those fast first 50 → not representative

### Solution

Add `--warmup-percent` flag (default 10%) to exclude first N% of completed executions from latency metrics.

**Add to CLI** (`core/src/cli.rs` around line 122):
```rust
/// Warmup percentage: exclude first N% of executions from latency metrics
#[arg(long, default_value = "10")]
warmup_percent: f64,
```

**Add to BenchmarkParams:**
```rust
pub struct BenchmarkParams {
    // ... existing fields ...
    pub warmup_percent: f64,
}
```

**Modify collect_metrics query:**
```rust
// Calculate warmup cutoff: exclude first N% of completed executions by completed_at
WITH ranked AS (
    SELECT
        *,
        EXTRACT(EPOCH FROM (completed_at - created_at)) * 1000 as latency_ms,
        ROW_NUMBER() OVER (ORDER BY completed_at) as rn,
        COUNT(*) OVER () as total
    FROM executions
    WHERE created_at >= $1
      AND created_at <= $2
      AND status = 'completed'
),
warmup_cutoff AS (
    SELECT CEIL(MAX(total) * $3 / 100.0) as cutoff_row
    FROM ranked
)
SELECT
    COUNT(*) as completed_count,
    AVG(latency_ms) as avg,
    percentile_cont(0.5) WITHIN GROUP (ORDER BY latency_ms) as p50,
    percentile_cont(0.95) WITHIN GROUP (ORDER BY latency_ms) as p95,
    percentile_cont(0.99) WITHIN GROUP (ORDER BY latency_ms) as p99
FROM ranked
WHERE rn > (SELECT cutoff_row FROM warmup_cutoff)
```

Bind `warmup_percent` as $3.

**Display warmup info:**
```
📊 Benchmark Results (10% warmup excluded)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📋 Work:
   Enqueued: 1000
   Completed: 995
   Warmup excluded: 100 (10%)
   Measured: 895
```

### Files to change
- `core/src/cli.rs` - Add warmup_percent argument
- `core/src/benchmark.rs` - Update BenchmarkParams, collect_metrics query, display_report

---

## 6. Add JSON/CSV Output Format for CI Integration

**Status:** Not started
**Priority:** MEDIUM (CI integration)
**Complexity:** Low (30 lines)

### Problem

Current output is human-readable only. For CI/CD pipelines, you want machine-readable output:
```bash
result=$(currant bench --format json | jq '.throughput')
if (( $(echo "$result < 100" | bc -l) )); then
    echo "Performance regression detected!"
    exit 1
fi
```

### Solution

Add `--format` flag with options: `human` (default), `json`, `csv`.

**Add to CLI:**
```rust
/// Output format
#[arg(long, default_value = "human")]
format: String,
```

**Add to BenchmarkParams:**
```rust
pub format: String,
```

**Modify display_report:**
```rust
fn display_report(metrics: &BenchmarkMetrics, format: &str) {
    match format {
        "json" => display_json(metrics),
        "csv" => display_csv(metrics),
        _ => display_human(metrics),
    }
}

fn display_json(metrics: &BenchmarkMetrics) {
    use serde_json::json;

    let output = json!({
        "duration_secs": (metrics.end_time - metrics.start_time).num_milliseconds() as f64 / 1000.0,
        "enqueued": {
            "jobs": metrics.enqueued_jobs,
            "workflows": metrics.enqueued_workflows,
            "total": metrics.enqueued_jobs + metrics.enqueued_workflows,
        },
        "completed": metrics.completed_jobs,
        "failed": metrics.failed_jobs,
        "pending": metrics.pending_jobs,
        "throughput_per_sec": /* calculate */,
        "latency_ms": {
            "avg": metrics.avg_latency_ms,
            "p50": metrics.p50_latency_ms,
            "p95": metrics.p95_latency_ms,
            "p99": metrics.p99_latency_ms,
        }
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn display_csv(metrics: &BenchmarkMetrics) {
    println!("metric,value");
    println!("duration_secs,{:.2}", /* ... */);
    println!("throughput_per_sec,{:.1}", /* ... */);
    println!("avg_latency_ms,{:.1}", metrics.avg_latency_ms);
    // ... etc
}
```

**Note:** BenchmarkMetrics struct may need to derive Serialize if we want clean JSON serialization.

### Files to change
- `core/src/cli.rs` - Add format argument
- `core/src/benchmark.rs` - Add format to params, modify display_report, add display_json/display_csv functions
- `core/Cargo.toml` - Ensure serde_json is available (already is)

---

## 7. Add Optional Verbose Mode for Worker stdout/stderr

**Status:** Not started
**Priority:** MEDIUM (debugging)
**Complexity:** Low (10 lines)

### Problem

Workers currently have stdout/stderr suppressed (line 204-205):
```rust
.stdout(Stdio::null())
.stderr(Stdio::null())
```

When debugging why jobs are failing or performing poorly, you need to see worker output.

### Solution

Add `--verbose` flag that preserves worker output.

**Add to CLI:**
```rust
/// Show worker output (useful for debugging)
#[arg(long, short = 'v')]
verbose: bool,
```

**Add to BenchmarkParams:**
```rust
pub verbose: bool,
```

**Modify spawn_workers:**
```rust
fn spawn_workers(count: usize, queues: &str, verbose: bool) -> Result<Vec<Child>> {
    // ... existing code ...

    let worker = cmd
        .stdout(if verbose { Stdio::inherit() } else { Stdio::null() })
        .stderr(if verbose { Stdio::inherit() } else { Stdio::null() })
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn worker {}: {}", i, e))?;

    // ...
}
```

**Call site update** (line 65):
```rust
let workers = spawn_workers(params.workers, &params.queues, params.verbose)?;
```

### Files to change
- `core/src/cli.rs` - Add verbose flag
- `core/src/benchmark.rs` - Add to params, modify spawn_workers signature and implementation

---

## 8. Add Retry Statistics and Failure Analysis

**Status:** Not started
**Priority:** MEDIUM (reliability insights)
**Complexity:** Medium (40 lines)

### Problem

Current metrics show "completed" vs "failed" but don't show:
- How many jobs succeeded on first attempt vs required retries
- Average retry count
- What percentage of failures were due to max retries vs other reasons

This information is valuable for understanding system reliability.

### Solution

The `executions` table has `attempt` column (validated: exists, values start at 1).

**Add to BenchmarkMetrics:**
```rust
struct RetryStats {
    first_attempt_success: i64,
    succeeded_after_retry: i64,
    max_retries_exceeded: i64,
    avg_attempts: f64,
}

struct BenchmarkMetrics {
    // ... existing ...
    retry_stats: RetryStats,
}
```

**Add query to collect_metrics:**
```sql
-- Retry statistics
SELECT
    COUNT(*) FILTER (WHERE status = 'completed' AND attempt = 1) as first_attempt_success,
    COUNT(*) FILTER (WHERE status = 'completed' AND attempt > 1) as succeeded_after_retry,
    COUNT(*) FILTER (WHERE status = 'failed' AND attempt >= max_retries) as max_retries_exceeded,
    AVG(attempt) FILTER (WHERE status = 'completed') as avg_attempts
FROM executions
WHERE created_at >= $1 AND created_at <= $2
```

Execute this as a separate query or combine with main query.

**Display:**
```
📊 Reliability:
   First-attempt success: 945 (94.5%)
   Succeeded after retry: 50 (5.0%)
   Failed (max retries): 5 (0.5%)
   Average attempts: 1.07
```

**For JSON output:**
```json
{
  "retry_stats": {
    "first_attempt_success_count": 945,
    "first_attempt_success_percent": 94.5,
    "succeeded_after_retry": 50,
    "max_retries_exceeded": 5,
    "avg_attempts": 1.07
  }
}
```

### Files to change
- `core/src/benchmark.rs` - Add RetryStats struct, modify BenchmarkMetrics, add retry query to collect_metrics, update display functions

---

## Implementation Order

Suggested order to maximize value and minimize conflicts:

1. **Queue distribution fix** - Critical bug, isolated change
2. **Cleanup guard** - Safety feature, doesn't conflict with metrics changes
3. **Percentile metrics** - Foundation for remaining metrics improvements
4. **Job/workflow separation** - Builds on percentile metrics
5. **Warmup period** - Builds on percentile metrics
6. **Retry statistics** - Independent metrics addition
7. **Verbose mode** - Simple, independent
8. **JSON output** - Should be last, depends on all metric struct changes

---

## Testing Checklist

After ALL changes are complete, test:

```bash
# Basic functionality
currant bench --workers 5 --jobs 100

# Multi-queue with distribution
currant bench --workers 5 --jobs 1000 --queues q1,q2,q3 --queue-distribution 50,30,20

# Workflows
currant bench --workers 3 --workflows 50 --activities-per-workflow 5

# JSON output
currant bench --workers 2 --jobs 100 --format json | jq .

# CSV output
currant bench --workers 2 --jobs 100 --format csv

# Verbose mode
currant bench --workers 1 --jobs 10 --verbose

# Warmup
currant bench --workers 5 --jobs 1000 --warmup-percent 20

# Error handling (workers should be cleaned up)
# Modify code to inject error after worker spawn, verify cleanup
```

---

## Notes

- All PostgreSQL queries tested against postgres:16-alpine
- percentile_cont() function confirmed working
- executions table schema confirmed via `\d executions`
- worker_heartbeats table exists but not used for this work
- Python workers spawn via `python -m currant worker --import currant.benchmark`
- Benchmark is language-specific (Python) by design - tests full stack including FFI

## Dependencies

Current dependencies in core/Cargo.toml that we'll use:
- sqlx (database queries)
- serde_json (JSON output)
- anyhow (error handling)
- tokio (async runtime)
- clap (CLI parsing)
- chrono (timestamps)

No new dependencies required (unless we choose scopeguard for cleanup guard).
