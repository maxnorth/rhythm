use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "currant")]
#[command(about = "Currant - A lightweight durable execution framework", long_about = None)]
pub struct Cli {
    /// Path to config file (overrides default search)
    #[arg(long, global = true)]
    pub config: Option<String>,

    /// Database URL (overrides config file and env vars)
    #[arg(long, global = true)]
    pub database_url: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Get the status of an execution
    Status {
        /// Execution ID to query
        execution_id: String,
    },

    /// List executions
    List {
        /// Filter by queue
        #[arg(short = 'q', long = "queue")]
        queue: Option<String>,

        /// Filter by status
        #[arg(short = 's', long = "status")]
        status: Option<String>,

        /// Number of results (default: 20)
        #[arg(short = 'l', long = "limit", default_value = "20")]
        limit: i32,
    },

    /// Cancel a pending or suspended execution
    Cancel {
        /// Execution ID to cancel
        execution_id: String,

        /// Skip confirmation prompt
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },

    /// Send a signal to a workflow
    Signal {
        /// Workflow execution ID
        workflow_id: String,

        /// Signal name
        signal_name: String,

        /// Signal payload (JSON string)
        #[arg(default_value = "{}")]
        payload: String,
    },

    /// Run baseline benchmark (pure Rust, no external workers)
    Bench {
        /// Number of concurrent tokio tasks
        #[arg(long, default_value = "100")]
        concurrency: usize,

        /// Microseconds of simulated work per task
        #[arg(long)]
        work_delay_us: Option<u64>,

        /// Number of tasks to enqueue
        #[arg(long, default_value = "0")]
        tasks: usize,

        /// Number of workflows to enqueue
        #[arg(long, default_value = "0")]
        workflows: usize,

        /// Task type: noop, compute
        #[arg(long, default_value = "noop")]
        task_type: String,

        /// Payload size in bytes
        #[arg(long, default_value = "0")]
        payload_size: usize,

        /// Tasks per workflow
        #[arg(long, default_value = "3")]
        tasks_per_workflow: usize,

        /// Queues to use (comma-separated)
        #[arg(long, default_value = "default")]
        queues: String,

        /// Queue distribution as percentages (comma-separated, must sum to 100)
        #[arg(long)]
        queue_distribution: Option<String>,

        /// Benchmark duration (e.g., "60s", "5m")
        #[arg(long)]
        duration: Option<String>,

        /// Target task enqueue rate (tasks/sec)
        #[arg(long)]
        rate: Option<f64>,

        /// Compute iterations for compute task type
        #[arg(long, default_value = "1000")]
        compute_iterations: usize,

        /// Warmup percentage: exclude first N% of executions from latency metrics
        #[arg(long, default_value = "0")]
        warmup_percent: f64,
    },
}

/// Run the CLI by parsing process arguments
/// This function is meant to be called from language adapters
pub async fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    run_cli_with_args(cli).await
}

/// Run the CLI with provided arguments (for language adapters that need to filter args)
pub async fn run_cli_from_args(args: Vec<String>) -> Result<()> {
    let cli = Cli::parse_from(args);
    run_cli_with_args(cli).await
}

/// Internal function that handles CLI commands
async fn run_cli_with_args(cli: Cli) -> Result<()> {
    use crate::config::Config;
    use crate::executions;
    use crate::signals;
    use std::env;

    // Apply CLI overrides to environment before any database operations
    if let Some(config_path) = &cli.config {
        env::set_var("CURRANT_CONFIG_PATH", config_path);
    }
    if let Some(database_url) = &cli.database_url {
        env::set_var("CURRANT_DATABASE_URL", database_url);
    }

    // Eagerly load and validate configuration before executing any command
    // This ensures config errors are shown immediately, not after command output
    let _ = Config::load()?;

    match cli.command {
        Commands::Status { execution_id } => {
            match executions::get_execution(&execution_id).await? {
                Some(exec) => {
                    println!("Execution: {}", exec.id);
                    println!("Type: {:?}", exec.exec_type);
                    println!("Function: {}", exec.function_name);
                    println!("Queue: {}", exec.queue);
                    println!("Status: {:?}", exec.status);
                    println!("Priority: {}", exec.priority);
                    println!("Attempts: {}/{}", exec.attempt, exec.max_retries);
                    println!("Created: {}", exec.created_at);

                    if let Some(claimed_at) = exec.claimed_at {
                        println!("Claimed: {}", claimed_at);
                    }
                    if let Some(completed_at) = exec.completed_at {
                        println!("Completed: {}", completed_at);
                    }

                    if let Some(result) = exec.result {
                        println!("\nResult:");
                        println!("  {}", result);
                    }

                    if let Some(error) = exec.error {
                        println!("\nError:");
                        println!("  {}", error);
                    }
                }
                None => {
                    eprintln!("Execution {} not found", execution_id);
                    std::process::exit(1);
                }
            }
        }

        Commands::List {
            queue,
            status,
            limit,
        } => {
            use crate::types::{ExecutionListFilter, ExecutionStatus};

            // Parse status string to enum if provided
            let status_filter = if let Some(s) = status {
                let status_enum = match s.to_lowercase().as_str() {
                    "pending" => ExecutionStatus::Pending,
                    "running" => ExecutionStatus::Running,
                    "suspended" => ExecutionStatus::Suspended,
                    "completed" => ExecutionStatus::Completed,
                    "failed" => ExecutionStatus::Failed,
                    _ => {
                        eprintln!("Invalid status: {}. Must be one of: pending, running, suspended, completed, failed", s);
                        std::process::exit(1);
                    }
                };
                Some(status_enum)
            } else {
                None
            };

            let filter = ExecutionListFilter {
                queue,
                status: status_filter,
                limit: Some(limit),
            };

            let executions = executions::list_executions(filter).await?;

            if executions.is_empty() {
                println!("No executions found");
                return Ok(());
            }

            println!("Found {} execution(s):\n", executions.len());

            for exec in executions {
                let id_short = if exec.id.len() > 12 {
                    &exec.id[..12]
                } else {
                    &exec.id
                };
                println!(
                    "  {}... | {:?} | {:?} | {} | {}",
                    id_short, exec.exec_type, exec.status, exec.queue, exec.function_name
                );
            }
        }

        Commands::Cancel { execution_id, yes } => {
            if !yes {
                eprintln!("Error: Confirmation required. Use --yes flag to confirm cancellation.");
                eprintln!("Note: Interactive prompts should be handled by language adapters");
                std::process::exit(1);
            }

            executions::cancel_execution(&execution_id).await?;
            println!("✓ Execution {} cancelled", execution_id);
        }

        Commands::Signal {
            workflow_id,
            signal_name,
            payload,
        } => {
            // Parse JSON payload
            let payload_value: serde_json::Value = serde_json::from_str(&payload)?;

            let signal_id =
                signals::send_signal(&workflow_id, &signal_name, payload_value).await?;

            println!("✓ Signal sent: {}", signal_id);
        }

        Commands::Bench {
            concurrency,
            work_delay_us,
            tasks,
            workflows,
            task_type,
            payload_size,
            tasks_per_workflow,
            queues,
            queue_distribution,
            duration,
            rate,
            compute_iterations,
            warmup_percent,
        } => {
            use crate::benchmark;

            let params = benchmark::BenchmarkParams {
                mode: benchmark::WorkerMode::Baseline {
                    concurrency,
                    work_delay_us,
                },
                tasks,
                workflows,
                task_type,
                payload_size,
                tasks_per_workflow,
                queues,
                queue_distribution,
                duration,
                rate,
                compute_iterations,
                warmup_percent,
            };

            benchmark::run_benchmark(params).await?;
        }
    }

    Ok(())
}
