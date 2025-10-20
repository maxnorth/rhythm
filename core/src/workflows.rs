use anyhow::{Context, Result};
use sha2::{Sha256, Digest};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::db;
use crate::interpreter;
use crate::types::{ExecutionType, ExecutionStatus};

// Re-export executor functions for convenience
pub use crate::interpreter::executor::{execute_workflow_step, StepResult};

/// Represents a workflow file from a language adapter
#[derive(Debug, Clone)]
pub struct WorkflowFile {
    pub name: String,
    pub source: String,
    pub file_path: String,
}

/// Register workflows during initialization
///
/// This function:
/// 1. Parses each workflow source to JSON steps
/// 2. Hashes the source to create a version
/// 3. Stores in workflow_definitions table
pub async fn register_workflows(workflows: Vec<WorkflowFile>) -> Result<()> {
    if workflows.is_empty() {
        return Ok(());
    }

    let pool = db::get_pool().await?;
    let workflow_count = workflows.len();

    for workflow in workflows {
        // Parse workflow source to JSON steps
        let steps = interpreter::parse_workflow(&workflow.source)
            .with_context(|| format!("Failed to parse workflow '{}' from {}", workflow.name, workflow.file_path))?;

        // Hash the source to create version
        let version_hash = hash_source(&workflow.source);

        // Serialize parsed steps to JSON
        let parsed_steps = serde_json::to_value(&steps)
            .context("Failed to serialize parsed steps")?;

        println!(
            "Registering workflow '{}' (version: {}, {} steps)",
            workflow.name,
            &version_hash[..8], // Show first 8 chars of hash
            steps.len()
        );

        // Store in database with parsed steps cached
        sqlx::query(
            r#"
            INSERT INTO workflow_definitions (name, version_hash, source, parsed_steps, file_path)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (name, version_hash) DO NOTHING
            "#,
        )
        .bind(&workflow.name)
        .bind(&version_hash)
        .bind(&workflow.source)
        .bind(&parsed_steps)
        .bind(&workflow.file_path)
        .execute(pool.as_ref())
        .await
        .with_context(|| format!("Failed to store workflow '{}'", workflow.name))?;
    }

    println!("Successfully registered {} workflow(s)", workflow_count);
    Ok(())
}

/// Hash workflow source using SHA256
fn hash_source(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Start a workflow execution
///
/// This function:
/// 1. Looks up the workflow definition by name (latest version)
/// 2. Creates an execution record with type=workflow
/// 3. Creates a workflow_execution_context record with initial state
/// 4. Returns the execution ID
pub async fn start_workflow(
    workflow_name: &str,
    inputs: JsonValue,
) -> Result<String> {
    let pool = db::get_pool().await?;

    // 1. Look up workflow definition by name (get latest version)
    let workflow_def: Option<(i32,)> = sqlx::query_as(
        r#"
        SELECT id
        FROM workflow_definitions
        WHERE name = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(workflow_name)
    .fetch_optional(pool.as_ref())
    .await
    .context("Failed to query workflow definition")?;

    let (workflow_def_id,) = workflow_def
        .ok_or_else(|| anyhow::anyhow!("Workflow '{}' not found", workflow_name))?;

    // 2. Create execution record
    let execution_id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO executions (
            id, type, function_name, queue, status, priority,
            args, kwargs, options, max_retries, timeout_seconds
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(&execution_id)
    .bind(&ExecutionType::Workflow)
    .bind(workflow_name)
    .bind("default") // Use default queue
    .bind(&ExecutionStatus::Pending)
    .bind(5) // Default priority
    .bind(serde_json::json!([])) // Empty args
    .bind(&inputs) // inputs go in kwargs
    .bind(serde_json::json!({})) // Empty options
    .bind(0) // No retries for workflows
    .bind(None::<i32>) // No timeout
    .execute(pool.as_ref())
    .await
    .context("Failed to create workflow execution")?;

    // 3. Create workflow execution context with initial state
    sqlx::query(
        r#"
        INSERT INTO workflow_execution_context (
            execution_id, workflow_definition_id,
            statement_index, locals, awaiting_task_id
        ) VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(&execution_id)
    .bind(workflow_def_id)
    .bind(0) // Start at statement 0
    .bind(serde_json::json!({})) // Empty locals initially
    .bind(None::<String>) // Not awaiting any task
    .execute(pool.as_ref())
    .await
    .context("Failed to create workflow execution context")?;

    // 4. Send notification to default queue
    sqlx::query("NOTIFY \"default\", $1")
        .bind(&execution_id)
        .execute(pool.as_ref())
        .await
        .ok(); // Don't fail if notify fails

    Ok(execution_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_source() {
        let source = "task(\"test\", {})";
        let hash = hash_source(source);

        assert_eq!(hash.len(), 64); // SHA256 produces 64 hex chars

        // Same source should produce same hash
        assert_eq!(hash, hash_source(source));

        // Different source should produce different hash
        let different = "task(\"other\", {})";
        assert_ne!(hash, hash_source(different));
    }

    #[tokio::test]
    async fn test_register_empty_workflows() {
        let result = register_workflows(vec![]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_register_workflow_parsing() {
        let workflow = WorkflowFile {
            name: "test".to_string(),
            source: r#"task("myTask", { "input": "value" })"#.to_string(),
            file_path: "test.flow".to_string(),
        };

        // This should parse successfully (but not store, since DB not connected in test)
        let result = register_workflows(vec![workflow]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_register_workflow_parse_error() {
        let workflow = WorkflowFile {
            name: "invalid".to_string(),
            source: "invalid syntax here".to_string(),
            file_path: "invalid.flow".to_string(),
        };

        let result = register_workflows(vec![workflow]).await;
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to parse workflow"));
    }
}
