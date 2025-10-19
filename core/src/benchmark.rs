use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde_json::json;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::db::get_pool;
use crate::executions::{claim_execution, complete_execution, create_execution};
use crate::types::{CreateExecutionParams, ExecutionType};

/// Worker mode for benchmarking
pub enum WorkerMode {
    /// Spawn external worker processes (language adapters)
    External {
        command: Vec<String>,
        workers: usize,
    },
    /// Run baseline benchmark with internal tokio tasks (no external processes)
    Baseline {
        concurrency: usize,
        work_delay_us: Option<u64>,
    },
}

pub struct BenchmarkParams {
    pub mode: WorkerMode,
    pub tasks: usize,
    pub workflows: usize,
    pub task_type: String,
    pub payload_size: usize,
    pub tasks_per_workflow: usize,
    pub queues: String,
    pub queue_distribution: Option<String>,
    pub duration: Option<String>,
    pub rate: Option<f64>,
    pub compute_iterations: usize,
    pub warmup_percent: f64,
}

struct LatencyMetrics {
    count: i64,
    count_after_warmup: i64,
    avg_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
}

impl Default for LatencyMetrics {
    fn default() -> Self {
        Self {
            count: 0,
            count_after_warmup: 0,
            avg_ms: 0.0,
            p50_ms: 0.0,
            p95_ms: 0.0,
            p99_ms: 0.0,
        }
    }
}

struct BenchmarkMetrics {
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    enqueued_tasks: usize,
    enqueued_workflows: usize,
    completed_tasks: i64,
    failed_tasks: i64,
    pending_tasks: i64,
    task_latency: LatencyMetrics,
    workflow_latency: LatencyMetrics,
}

/// RAII guard that ensures workers are stopped when dropped
/// This prevents orphaned worker processes if the benchmark fails
struct WorkerGuard {
    workers: Option<Vec<Child>>,
}

impl WorkerGuard {
    fn new(workers: Vec<Child>) -> Self {
        Self {
            workers: Some(workers),
        }
    }

    /// Extract workers for normal shutdown, preventing drop cleanup
    fn take(mut self) -> Vec<Child> {
        self.workers.take().unwrap_or_default()
    }
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        if let Some(workers) = self.workers.take() {
            eprintln!("\n⚠️  Cleaning up {} workers due to early exit", workers.len());
            let _ = stop_workers(workers);
        }
    }
}

pub async fn run_benchmark(params: BenchmarkParams) -> Result<()> {
    println!("🚀 Starting Currant Benchmark");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Validate parameters
    validate_params(&params)?;

    let queues: Vec<&str> = params.queues.split(',').collect();
    let distribution = parse_distribution(&params.queue_distribution, queues.len())?;

    // Display configuration based on mode
    println!("\n📋 Configuration:");
    match &params.mode {
        WorkerMode::External { command: _, workers } => {
            println!("   Mode: External Workers");
            println!("   Workers: {}", workers);
        }
        WorkerMode::Baseline { concurrency, work_delay_us } => {
            println!("   Mode: Baseline (Pure Rust)");
            println!("   Concurrency: {} tokio tasks", concurrency);
            if let Some(delay) = work_delay_us {
                println!("   Work Delay: {}μs", delay);
            }
        }
    }
    println!("   Tasks: {}", params.tasks);
    println!("   Workflows: {}", params.workflows);
    println!("   Task Type: {}", params.task_type);
    if params.payload_size > 0 {
        println!("   Payload Size: {} bytes", params.payload_size);
    }
    if params.workflows > 0 {
        println!("   Activities/Workflow: {}", params.tasks_per_workflow);
    }
    println!("   Queues: {}", params.queues);
    if let Some(ref rate) = params.rate {
        println!("   Enqueue Rate: {}/sec", rate);
    }

    // Step 1: Start workers based on mode
    let _worker_guard = match &params.mode {
        WorkerMode::External { command, workers } => {
            println!("\n🔧 Spawning {} external workers...", workers);
            let spawned_workers = spawn_workers(command, *workers, &params.queues)?;
            println!("   Waiting for workers to initialize...");
            sleep(Duration::from_secs(8)).await;
            println!("✓ Workers started");
            Some(WorkerGuard::new(spawned_workers))
        }
        WorkerMode::Baseline { concurrency, work_delay_us } => {
            println!("\n🔧 Starting {} baseline workers (tokio tasks)...", concurrency);
            spawn_baseline_workers(*concurrency, &queues, *work_delay_us).await;
            println!("✓ Baseline workers started");
            None
        }
    };

    // Step 2: Enqueue tasks
    println!("\n📬 Enqueueing work...");
    let enqueue_start = Instant::now();

    // Mark the time when first taskis enqueued (for finding executions)
    let enqueue_start_time = Utc::now();

    let enqueued_tasks = enqueue_tasks(&params, &queues, &distribution).await?;
    let enqueued_workflows = enqueue_workflows(&params, &queues, &distribution).await?;

    let enqueue_duration = enqueue_start.elapsed();
    println!("✓ Enqueued {} tasks and {} workflows in {:.2}s",
             enqueued_tasks, enqueued_workflows, enqueue_duration.as_secs_f64());

    // Start measuring execution performance AFTER enqueueing
    let _execution_start_time = Utc::now();

    // Step 3: Wait for completion or timeout
    println!("\n⏳ Waiting for tasks to complete...");
    let total_work = enqueued_tasks + enqueued_workflows;

    let timeout = if let Some(duration_str) = &params.duration {
        parse_duration(duration_str)?
    } else {
        Duration::from_secs(300) // 5 minute default timeout
    };

    wait_for_completion(enqueue_start_time, total_work, timeout).await?;

    // Give workflows a moment to finalize after their tasks complete
    // This ensures workflow executions themselves show as 'completed' in metrics
    if enqueued_workflows > 0 {
        sleep(Duration::from_millis(500)).await;
    }

    let end_time = Utc::now();

    // Step 4: Collect metrics
    println!("\n📊 Collecting metrics...");
    let metrics = collect_metrics(
        enqueue_start_time,    // For finding which executions to count and calculating throughput
        end_time,
        enqueued_tasks,
        enqueued_workflows,
        params.warmup_percent
    ).await?;

    // Step 5: Stop workers (only for external mode)
    if let Some(guard) = _worker_guard {
        println!("\n🛑 Stopping workers...");
        let workers = guard.take();
        stop_workers(workers)?;
        println!("✓ Workers stopped");
    }
    // Note: Baseline workers (tokio tasks) will be dropped automatically

    // Step 6: Display report
    display_report(&metrics, params.warmup_percent);

    Ok(())
}

fn validate_params(params: &BenchmarkParams) -> Result<()> {
    if params.tasks == 0 && params.workflows == 0 {
        return Err(anyhow!("Must specify --tasks or --workflows (or both)"));
    }

    // Validate mode-specific parameters
    match &params.mode {
        WorkerMode::External { command, workers } => {
            if command.is_empty() {
                return Err(anyhow!("Worker command cannot be empty"));
            }
            if *workers == 0 {
                return Err(anyhow!("Must have at least 1 worker"));
            }
        }
        WorkerMode::Baseline { concurrency, work_delay_us: _ } => {
            if *concurrency == 0 {
                return Err(anyhow!("Concurrency must be at least 1"));
            }
        }
    }

    match params.task_type.as_str() {
        "noop" | "compute" => {},
        _ => return Err(anyhow!("Invalid task type '{}'. Must be 'noop' or 'compute'", params.task_type)),
    }

    Ok(())
}

fn parse_distribution(distribution: &Option<String>, num_queues: usize) -> Result<Vec<f64>> {
    if let Some(dist_str) = distribution {
        let percentages: Result<Vec<f64>> = dist_str
            .split(',')
            .map(|s| s.trim().parse::<f64>().map_err(|e| anyhow!("Invalid percentage: {}", e)))
            .collect();

        let percentages = percentages?;

        if percentages.len() != num_queues {
            return Err(anyhow!("Queue distribution must have {} values (one per queue)", num_queues));
        }

        let sum: f64 = percentages.iter().sum();
        if (sum - 100.0).abs() > 0.01 {
            return Err(anyhow!("Queue distribution must sum to 100 (got {})", sum));
        }

        Ok(percentages.iter().map(|p| p / 100.0).collect())
    } else {
        // Equal distribution
        Ok(vec![1.0 / num_queues as f64; num_queues])
    }
}

fn parse_duration(duration_str: &str) -> Result<Duration> {
    let duration_str = duration_str.trim();

    if duration_str.ends_with("ms") {
        let ms: u64 = duration_str.trim_end_matches("ms").parse()?;
        Ok(Duration::from_millis(ms))
    } else if duration_str.ends_with('s') {
        let secs: u64 = duration_str.trim_end_matches('s').parse()?;
        Ok(Duration::from_secs(secs))
    } else if duration_str.ends_with('m') {
        let mins: u64 = duration_str.trim_end_matches('m').parse()?;
        Ok(Duration::from_secs(mins * 60))
    } else {
        Err(anyhow!("Invalid duration format. Use '60s', '5m', etc."))
    }
}

/// Spawn baseline workers (tokio tasks, not external processes)
async fn spawn_baseline_workers(concurrency: usize, queues: &[&str], work_delay_us: Option<u64>) {
    let queues_vec: Vec<String> = queues.iter().map(|s| s.to_string()).collect();

    for _ in 0..concurrency {
        let queues_clone = queues_vec.clone();
        tokio::spawn(async move {
            loop {
                // Claim a task
                match claim_execution("baseline-worker", &queues_clone).await {
                    Ok(Some(exec)) => {
                        // Optional work delay to simulate processing
                        if let Some(delay_us) = work_delay_us {
                            tokio::time::sleep(Duration::from_micros(delay_us)).await;
                        }

                        // Complete the task immediately
                        let result = json!({"status": "completed", "mode": "baseline"});
                        if let Err(e) = complete_execution(&exec.id, result).await {
                            eprintln!("Baseline worker error completing execution: {}", e);
                        }
                    }
                    Ok(None) => {
                        // No work available, sleep briefly
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    Err(e) => {
                        eprintln!("Baseline worker error claiming execution: {}", e);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        });
    }
}

fn spawn_workers(worker_command: &[String], count: usize, queues: &str) -> Result<Vec<Child>> {
    let mut workers = Vec::new();

    if worker_command.is_empty() {
        return Err(anyhow!("Worker command cannot be empty"));
    }

    for i in 0..count {
        let mut cmd = Command::new(&worker_command[0]);

        // Add remaining command arguments
        if worker_command.len() > 1 {
            cmd.args(&worker_command[1..]);
        }

        // Add queue arguments
        cmd.args(["--queue", queues]);

        let worker = cmd
            .stdout(Stdio::inherit()) // Show worker output for debugging
            .stderr(Stdio::inherit()) // Show worker errors for debugging
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn worker {}: {}", i, e))?;

        workers.push(worker);
    }

    Ok(workers)
}

async fn enqueue_tasks(
    params: &BenchmarkParams,
    queues: &[&str],
    distribution: &[f64],
) -> Result<usize> {
    if params.tasks == 0 {
        return Ok(0);
    }

    let function_name = match params.task_type.as_str() {
        "noop" => "currant.benchmark.__currant_bench_noop__",
        "compute" => "currant.benchmark.__currant_bench_compute__",
        _ => return Err(anyhow!("Unknown tasktype")),
    };

    let mut kwargs = serde_json::Map::new();
    if params.payload_size > 0 {
        kwargs.insert("payload_size".to_string(), json!(params.payload_size));
    }
    if params.task_type == "compute" {
        kwargs.insert("iterations".to_string(), json!(params.compute_iterations));
    }

    for i in 0..params.tasks {
        let queue = select_queue(queues, distribution, i);

        create_execution(CreateExecutionParams {
            id: None,
            exec_type: ExecutionType::Task,
            function_name: function_name.to_string(),
            queue: queue.to_string(),
            priority: 5,
            args: json!([]),
            kwargs: serde_json::Value::Object(kwargs.clone()),
            max_retries: 3,
            timeout_seconds: Some(30),
            parent_workflow_id: None,
        })
        .await?;

        // Rate limiting
        if let Some(rate) = params.rate {
            sleep(Duration::from_secs_f64(1.0 / rate)).await;
        }
    }

    Ok(params.tasks)
}

async fn enqueue_workflows(
    params: &BenchmarkParams,
    queues: &[&str],
    distribution: &[f64],
) -> Result<usize> {
    if params.workflows == 0 {
        return Ok(0);
    }

    let mut kwargs = serde_json::Map::new();
    kwargs.insert("task_count".to_string(), json!(params.tasks_per_workflow));
    if params.payload_size > 0 {
        kwargs.insert("payload_size".to_string(), json!(params.payload_size));
    }

    for i in 0..params.workflows {
        let queue = select_queue(queues, distribution, i);

        create_execution(CreateExecutionParams {
            id: None,
            exec_type: ExecutionType::Workflow,
            function_name: "currant.benchmark.__currant_bench_workflow__".to_string(),
            queue: queue.to_string(),
            priority: 5,
            args: json!([]),
            kwargs: serde_json::Value::Object(kwargs.clone()),
            max_retries: 3,
            timeout_seconds: Some(60),
            parent_workflow_id: None,
        })
        .await?;

        // Rate limiting
        if let Some(rate) = params.rate {
            sleep(Duration::from_secs_f64(1.0 / rate)).await;
        }
    }

    Ok(params.workflows)
}

fn select_queue<'a>(queues: &[&'a str], distribution: &[f64], index: usize) -> &'a str {
    // Build cumulative distribution array [0.0, 0.5, 1.0] for 50/50 split
    // For each index, hash it to a pseudo-random value in [0, 1) and select queue
    // This ensures even distribution regardless of taskcount

    // Simple hash function to get deterministic pseudo-random value
    // Using index directly would cause clustering; we need to spread values uniformly
    let hash = (index.wrapping_mul(2654435761)) % 1000000;
    let random_value = hash as f64 / 1000000.0;

    // Select queue based on where random_value falls in cumulative distribution
    let mut cumulative = 0.0;
    for (i, &percentage) in distribution.iter().enumerate() {
        cumulative += percentage;
        if random_value < cumulative {
            return queues[i];
        }
    }

    // Fallback to last queue
    queues[queues.len() - 1]
}

async fn wait_for_completion(
    start_time: DateTime<Utc>,
    total_work: usize,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let pool = get_pool().await?;

    loop {
        if Instant::now() > deadline {
            println!("⚠️  Timeout reached");
            break;
        }

        // Check completion status
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM executions
            WHERE created_at >= $1
              AND status IN ('completed', 'failed')
            "#,
        )
        .bind(start_time)
        .fetch_one(pool.as_ref())
        .await?;

        let completed = row.0 as usize;
        let progress = (completed as f64 / total_work as f64 * 100.0).min(100.0);

        print!("\r   Progress: {}/{} ({:.1}%)", completed, total_work, progress);
        std::io::Write::flush(&mut std::io::stdout())?;

        if completed >= total_work {
            println!("\n✓ All work completed");
            break;
        }

        sleep(Duration::from_millis(500)).await;
    }

    Ok(())
}

async fn collect_metrics(
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    enqueued_tasks: usize,
    enqueued_workflows: usize,
    warmup_percent: f64,
) -> Result<BenchmarkMetrics> {
    let pool = get_pool().await?;

    // Collect completion counts
    let counts: (i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE status = 'completed') as completed,
            COUNT(*) FILTER (WHERE status = 'failed') as failed,
            COUNT(*) FILTER (WHERE status = 'pending') as pending
        FROM executions
        WHERE created_at >= $1 AND created_at <= $2
        "#,
    )
    .bind(start_time)
    .bind(end_time)
    .fetch_one(pool.as_ref())
    .await?;

    // Collect latency metrics grouped by type
    let latency_rows: Vec<(String, i64, i64, Option<f64>, Option<f64>, Option<f64>, Option<f64>)> = sqlx::query_as(
        r#"
        WITH latencies AS (
            SELECT
                type,
                EXTRACT(EPOCH FROM (completed_at - created_at)) * 1000 as latency_ms,
                ROW_NUMBER() OVER (PARTITION BY type ORDER BY completed_at) as row_num
            FROM executions
            WHERE created_at >= $1
              AND created_at <= $2
              AND status = 'completed'
        ),
        type_counts AS (
            SELECT
                type,
                COUNT(*) as total_count
            FROM latencies
            GROUP BY type
        ),
        warmup_filtered AS (
            SELECT
                l.type,
                l.latency_ms,
                tc.total_count,
                CASE
                    WHEN l.row_num > CEIL(tc.total_count * $3 / 100.0) THEN 1
                    ELSE 0
                END as after_warmup
            FROM latencies l
            JOIN type_counts tc ON l.type = tc.type
        )
        SELECT
            type,
            total_count,
            SUM(after_warmup) as count_after_warmup,
            CAST(AVG(CASE WHEN after_warmup = 1 THEN latency_ms END) AS DOUBLE PRECISION) as avg,
            CAST(percentile_cont(0.5) WITHIN GROUP (ORDER BY latency_ms) FILTER (WHERE after_warmup = 1) AS DOUBLE PRECISION) as p50,
            CAST(percentile_cont(0.95) WITHIN GROUP (ORDER BY latency_ms) FILTER (WHERE after_warmup = 1) AS DOUBLE PRECISION) as p95,
            CAST(percentile_cont(0.99) WITHIN GROUP (ORDER BY latency_ms) FILTER (WHERE after_warmup = 1) AS DOUBLE PRECISION) as p99
        FROM warmup_filtered
        GROUP BY type, total_count
        "#,
    )
    .bind(start_time)
    .bind(end_time)
    .bind(warmup_percent)
    .fetch_all(pool.as_ref())
    .await?;

    // Parse latency metrics by type
    let mut task_latency = LatencyMetrics::default();
    let mut workflow_latency = LatencyMetrics::default();

    for row in latency_rows {
        let metrics = LatencyMetrics {
            count: row.1,
            count_after_warmup: row.2,
            avg_ms: row.3.unwrap_or(0.0),
            p50_ms: row.4.unwrap_or(0.0),
            p95_ms: row.5.unwrap_or(0.0),
            p99_ms: row.6.unwrap_or(0.0),
        };

        match row.0.as_str() {
            "task" => task_latency = metrics,
            "workflow" => workflow_latency = metrics,
            _ => {}
        }
    }

    Ok(BenchmarkMetrics {
        start_time,
        end_time,
        enqueued_tasks,
        enqueued_workflows,
        completed_tasks: counts.0,
        failed_tasks: counts.1,
        pending_tasks: counts.2,
        task_latency,
        workflow_latency,
    })
}

fn stop_workers(mut workers: Vec<Child>) -> Result<()> {
    use std::time::Duration;

    // Send SIGTERM first for graceful shutdown
    for worker in workers.iter_mut() {
        #[cfg(unix)]
        {
            
            // Send SIGTERM (15) for graceful shutdown
            unsafe {
                libc::kill(worker.id() as i32, libc::SIGTERM);
            }
        }
        #[cfg(not(unix))]
        {
            let _ = worker.kill();
        }
    }

    // Give workers 2 seconds to shut down gracefully
    std::thread::sleep(Duration::from_secs(2));

    // Force kill any remaining workers
    for worker in workers.iter_mut() {
        let _ = worker.kill();
    }

    // Wait for all processes to exit
    for worker in workers.iter_mut() {
        let _ = worker.wait();
    }

    Ok(())
}

fn display_report(metrics: &BenchmarkMetrics, warmup_percent: f64) {
    let duration_secs = (metrics.end_time - metrics.start_time).num_milliseconds() as f64 / 1000.0;
    let total_enqueued = metrics.enqueued_tasks + metrics.enqueued_workflows;
    let total_completed = metrics.completed_tasks + metrics.failed_tasks;
    let throughput = total_completed as f64 / duration_secs;

    println!("\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📊 Benchmark Results");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("⏱️  Duration: {:.2}s", duration_secs);
    println!();
    println!("📋 Work:");
    println!("   Enqueued: {} tasks, {} workflows ({} total)",
             metrics.enqueued_tasks, metrics.enqueued_workflows, total_enqueued);
    println!("   Completed: {}", metrics.completed_tasks);
    println!("   Failed: {}", metrics.failed_tasks);
    println!("   Pending: {}", metrics.pending_tasks);
    println!();
    println!("🚀 Throughput: {:.1} tasks/sec", throughput);
    println!();

    // Display tasklatency if any tasks were run
    if metrics.task_latency.count > 0 {
        if warmup_percent > 0.0 {
            println!("📈 Job Latency ({} completed, {} after {:.0}% warmup):",
                     metrics.task_latency.count,
                     metrics.task_latency.count_after_warmup,
                     warmup_percent);
        } else {
            println!("📈 Job Latency ({} completed):", metrics.task_latency.count);
        }
        println!("   Average: {:.1}ms", metrics.task_latency.avg_ms);
        println!("   p50: {:.1}ms | p95: {:.1}ms | p99: {:.1}ms",
                 metrics.task_latency.p50_ms,
                 metrics.task_latency.p95_ms,
                 metrics.task_latency.p99_ms);
        println!();
    }

    // Display workflow latency if any workflows were run
    if metrics.workflow_latency.count > 0 {
        if warmup_percent > 0.0 {
            println!("📈 Workflow Latency ({} completed, {} after {:.0}% warmup):",
                     metrics.workflow_latency.count,
                     metrics.workflow_latency.count_after_warmup,
                     warmup_percent);
        } else {
            println!("📈 Workflow Latency ({} completed):", metrics.workflow_latency.count);
        }
        println!("   Average: {:.1}ms", metrics.workflow_latency.avg_ms);
        println!("   p50: {:.1}ms | p95: {:.1}ms | p99: {:.1}ms",
                 metrics.workflow_latency.p50_ms,
                 metrics.workflow_latency.p95_ms,
                 metrics.workflow_latency.p99_ms);
        println!();
    }

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
}
