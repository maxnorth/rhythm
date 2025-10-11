use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde_json::json;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::db::get_pool;
use crate::executions::create_execution;
use crate::types::{CreateExecutionParams, ExecutionType};

pub struct BenchmarkParams {
    pub workers: usize,
    pub jobs: usize,
    pub workflows: usize,
    pub job_type: String,
    pub payload_size: usize,
    pub activities_per_workflow: usize,
    pub queues: String,
    pub queue_distribution: Option<String>,
    pub duration: Option<String>,
    pub rate: Option<f64>,
    pub compute_iterations: usize,
}

struct LatencyMetrics {
    count: i64,
    avg_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
}

impl Default for LatencyMetrics {
    fn default() -> Self {
        Self {
            count: 0,
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
    enqueued_jobs: usize,
    enqueued_workflows: usize,
    completed_jobs: i64,
    failed_jobs: i64,
    pending_jobs: i64,
    job_latency: LatencyMetrics,
    workflow_latency: LatencyMetrics,
}

pub async fn run_benchmark(params: BenchmarkParams) -> Result<()> {
    println!("🚀 Starting Currant Benchmark");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Validate parameters
    validate_params(&params)?;

    let queues: Vec<&str> = params.queues.split(',').collect();
    let distribution = parse_distribution(&params.queue_distribution, queues.len())?;

    println!("\n📋 Configuration:");
    println!("   Workers: {}", params.workers);
    println!("   Jobs: {}", params.jobs);
    println!("   Workflows: {}", params.workflows);
    println!("   Job Type: {}", params.job_type);
    if params.payload_size > 0 {
        println!("   Payload Size: {} bytes", params.payload_size);
    }
    if params.workflows > 0 {
        println!("   Activities/Workflow: {}", params.activities_per_workflow);
    }
    println!("   Queues: {}", params.queues);
    if let Some(ref rate) = params.rate {
        println!("   Enqueue Rate: {}/sec", rate);
    }

    // Step 1: Spawn workers
    println!("\n🔧 Spawning {} workers...", params.workers);
    let workers = spawn_workers(params.workers, &params.queues)?;

    // Give workers time to start up (Python startup + import can be slow)
    println!("   Waiting for workers to initialize...");
    sleep(Duration::from_secs(8)).await;
    println!("✓ Workers started");

    // Step 2: Enqueue jobs
    println!("\n📬 Enqueueing work...");
    let enqueue_start = Instant::now();

    // Mark the time when first job is enqueued (for finding executions)
    let enqueue_start_time = Utc::now();

    let enqueued_jobs = enqueue_jobs(&params, &queues, &distribution).await?;
    let enqueued_workflows = enqueue_workflows(&params, &queues, &distribution).await?;

    let enqueue_duration = enqueue_start.elapsed();
    println!("✓ Enqueued {} jobs and {} workflows in {:.2}s",
             enqueued_jobs, enqueued_workflows, enqueue_duration.as_secs_f64());

    // Start measuring execution performance AFTER enqueueing
    let execution_start_time = Utc::now();

    // Step 3: Wait for completion or timeout
    println!("\n⏳ Waiting for jobs to complete...");
    let total_work = enqueued_jobs + enqueued_workflows;

    let timeout = if let Some(duration_str) = &params.duration {
        parse_duration(duration_str)?
    } else {
        Duration::from_secs(300) // 5 minute default timeout
    };

    wait_for_completion(enqueue_start_time, total_work, timeout).await?;

    // Give workflows a moment to finalize after their activities complete
    // This ensures workflow executions themselves show as 'completed' in metrics
    if enqueued_workflows > 0 {
        sleep(Duration::from_millis(500)).await;
    }

    let end_time = Utc::now();

    // Step 4: Collect metrics
    println!("\n📊 Collecting metrics...");
    let metrics = collect_metrics(
        enqueue_start_time,    // For finding which executions to count
        execution_start_time,  // For calculating throughput (exclude enqueueing time)
        end_time,
        enqueued_jobs,
        enqueued_workflows
    ).await?;

    // Step 5: Stop workers
    println!("\n🛑 Stopping workers...");
    stop_workers(workers)?;
    println!("✓ Workers stopped");

    // Step 6: Display report
    display_report(&metrics);

    Ok(())
}

fn validate_params(params: &BenchmarkParams) -> Result<()> {
    if params.jobs == 0 && params.workflows == 0 {
        return Err(anyhow!("Must specify --jobs or --workflows (or both)"));
    }

    if params.workers == 0 {
        return Err(anyhow!("Must have at least 1 worker"));
    }

    match params.job_type.as_str() {
        "noop" | "compute" => {},
        _ => return Err(anyhow!("Invalid job type '{}'. Must be 'noop' or 'compute'", params.job_type)),
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

fn spawn_workers(count: usize, queues: &str) -> Result<Vec<Child>> {
    let mut workers = Vec::new();

    // Try to detect Python executable - prefer current interpreter
    let python_cmd = std::env::var("PYTHON")
        .or_else(|_| std::env::var("VIRTUAL_ENV").map(|venv| format!("{}/bin/python", venv)))
        .unwrap_or_else(|_| {
            // Fall back to system python
            if Command::new("python3").arg("--version").output().is_ok() {
                "python3".to_string()
            } else {
                "python".to_string()
            }
        });

    for i in 0..count {
        let mut cmd = Command::new(&python_cmd);
        // Use -u flag for unbuffered output
        cmd.args(["-u", "-m", "currant", "worker", "--queue", queues, "--import", "currant.benchmark"]);

        let worker = cmd
            .stdout(Stdio::null()) // Suppress worker output
            .stderr(Stdio::null()) // Suppress worker errors
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn worker {}: {}", i, e))?;

        workers.push(worker);
    }

    Ok(workers)
}

async fn enqueue_jobs(
    params: &BenchmarkParams,
    queues: &[&str],
    distribution: &[f64],
) -> Result<usize> {
    if params.jobs == 0 {
        return Ok(0);
    }

    let function_name = match params.job_type.as_str() {
        "noop" => "currant.benchmark.__currant_bench_noop__",
        "compute" => "currant.benchmark.__currant_bench_compute__",
        _ => return Err(anyhow!("Unknown job type")),
    };

    let mut kwargs = serde_json::Map::new();
    if params.payload_size > 0 {
        kwargs.insert("payload_size".to_string(), json!(params.payload_size));
    }
    if params.job_type == "compute" {
        kwargs.insert("iterations".to_string(), json!(params.compute_iterations));
    }

    for i in 0..params.jobs {
        let queue = select_queue(queues, distribution, i);

        create_execution(CreateExecutionParams {
            exec_type: ExecutionType::Job,
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

    Ok(params.jobs)
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
    kwargs.insert("activity_count".to_string(), json!(params.activities_per_workflow));
    if params.payload_size > 0 {
        kwargs.insert("payload_size".to_string(), json!(params.payload_size));
    }

    for i in 0..params.workflows {
        let queue = select_queue(queues, distribution, i);

        create_execution(CreateExecutionParams {
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
    // This ensures even distribution regardless of job count

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
    enqueue_start_time: DateTime<Utc>,
    execution_start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    enqueued_jobs: usize,
    enqueued_workflows: usize,
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
    .bind(enqueue_start_time)
    .bind(end_time)
    .fetch_one(pool.as_ref())
    .await?;

    // Collect latency metrics grouped by type
    let latency_rows: Vec<(String, i64, Option<f64>, Option<f64>, Option<f64>, Option<f64>)> = sqlx::query_as(
        r#"
        WITH latencies AS (
            SELECT
                type,
                EXTRACT(EPOCH FROM (completed_at - created_at)) * 1000 as latency_ms
            FROM executions
            WHERE created_at >= $1
              AND created_at <= $2
              AND status = 'completed'
        )
        SELECT
            type,
            COUNT(*) as count,
            CAST(AVG(latency_ms) AS DOUBLE PRECISION) as avg,
            CAST(percentile_cont(0.5) WITHIN GROUP (ORDER BY latency_ms) AS DOUBLE PRECISION) as p50,
            CAST(percentile_cont(0.95) WITHIN GROUP (ORDER BY latency_ms) AS DOUBLE PRECISION) as p95,
            CAST(percentile_cont(0.99) WITHIN GROUP (ORDER BY latency_ms) AS DOUBLE PRECISION) as p99
        FROM latencies
        GROUP BY type
        "#,
    )
    .bind(enqueue_start_time)
    .bind(end_time)
    .fetch_all(pool.as_ref())
    .await?;

    // Parse latency metrics by type
    let mut job_latency = LatencyMetrics::default();
    let mut workflow_latency = LatencyMetrics::default();

    for row in latency_rows {
        let metrics = LatencyMetrics {
            count: row.1,
            avg_ms: row.2.unwrap_or(0.0),
            p50_ms: row.3.unwrap_or(0.0),
            p95_ms: row.4.unwrap_or(0.0),
            p99_ms: row.5.unwrap_or(0.0),
        };

        match row.0.as_str() {
            "job" => job_latency = metrics,
            "workflow" => workflow_latency = metrics,
            _ => {} // Ignore activities
        }
    }

    Ok(BenchmarkMetrics {
        start_time: execution_start_time,
        end_time,
        enqueued_jobs,
        enqueued_workflows,
        completed_jobs: counts.0,
        failed_jobs: counts.1,
        pending_jobs: counts.2,
        job_latency,
        workflow_latency,
    })
}

fn stop_workers(mut workers: Vec<Child>) -> Result<()> {
    use std::time::Duration;

    // Send SIGTERM first for graceful shutdown
    for worker in workers.iter_mut() {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
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

fn display_report(metrics: &BenchmarkMetrics) {
    let duration_secs = (metrics.end_time - metrics.start_time).num_milliseconds() as f64 / 1000.0;
    let total_enqueued = metrics.enqueued_jobs + metrics.enqueued_workflows;
    let total_completed = metrics.completed_jobs + metrics.failed_jobs;
    let throughput = total_completed as f64 / duration_secs;

    println!("\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📊 Benchmark Results");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("⏱️  Duration: {:.2}s", duration_secs);
    println!();
    println!("📋 Work:");
    println!("   Enqueued: {} jobs, {} workflows ({} total)",
             metrics.enqueued_jobs, metrics.enqueued_workflows, total_enqueued);
    println!("   Completed: {}", metrics.completed_jobs);
    println!("   Failed: {}", metrics.failed_jobs);
    println!("   Pending: {}", metrics.pending_jobs);
    println!();
    println!("🚀 Throughput: {:.1} jobs/sec", throughput);
    println!();

    // Display job latency if any jobs were run
    if metrics.job_latency.count > 0 {
        println!("📈 Job Latency ({} completed):", metrics.job_latency.count);
        println!("   Average: {:.1}ms", metrics.job_latency.avg_ms);
        println!("   p50: {:.1}ms | p95: {:.1}ms | p99: {:.1}ms",
                 metrics.job_latency.p50_ms,
                 metrics.job_latency.p95_ms,
                 metrics.job_latency.p99_ms);
        println!();
    }

    // Display workflow latency if any workflows were run
    if metrics.workflow_latency.count > 0 {
        println!("📈 Workflow Latency ({} completed):", metrics.workflow_latency.count);
        println!("   Average: {:.1}ms", metrics.workflow_latency.avg_ms);
        println!("   p50: {:.1}ms | p95: {:.1}ms | p99: {:.1}ms",
                 metrics.workflow_latency.p50_ms,
                 metrics.workflow_latency.p95_ms,
                 metrics.workflow_latency.p99_ms);
        println!();
    }

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
}
