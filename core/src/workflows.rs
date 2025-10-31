use anyhow::{Context, Result};
use sha2::{Sha256, Digest};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::db;
use crate::interpreter;
use crate::types::{ExecutionType, ExecutionStatus};

// Re-export interpreter functions for convenience
pub use crate::interpreter::executor::{execute_workflow_step, StepResult};
pub use crate::interpreter::parser::parse_workflow;

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
    use serde_json::json;

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
        let source = r#"workflow(ctx, inputs) {
    task("myTask", { "input": "value" })
}"#;

        // Just test that parsing works
        let result = parse_workflow(source);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_register_workflow_parse_error() {
        let source = "invalid syntax here";

        // Just test that parsing fails
        let result = parse_workflow(source);
        assert!(result.is_err());
    }

    // E2E Tests for various workflow syntax patterns

    #[tokio::test]
    async fn test_workflow_basic_sequential() {
        let source = r#"workflow(ctx, inputs) {
    await task("task1", { "step": 1 })
    await task("task2", { "step": 2 })
    await task("task3", { "step": 3 })
}"#;

        // Just test parsing, not registration (which requires DB)
        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 3, "Expected 3 statements");

        for (i, step) in parsed.iter().enumerate() {
            assert_eq!(step["type"], "task");
            assert_eq!(step["await"], true);
            assert_eq!(step["inputs"]["step"], i + 1);
        }
    }

    #[tokio::test]
    async fn test_workflow_fire_and_forget() {
        let source = r#"workflow(ctx, inputs) {
    task("task1", { "async": true })
    task("task2", { "async": true })
    task("task3", { "async": true })
}"#;

        // Just test parsing, not registration (which requires DB)
        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 3);

        for step in parsed.iter() {
            assert_eq!(step["type"], "task");
            assert_eq!(step["await"], false, "Non-awaited tasks should have await=false");
        }
    }

    #[tokio::test]
    async fn test_workflow_mixed_await() {
        let source = r#"workflow(ctx, inputs) {
    await task("init", { "step": 1 })
    task("background1", { "async": true })
    task("background2", { "async": true })
    await task("checkpoint", { "step": 2 })
    task("background3", { "async": true })
    await task("finalize", { "step": 3 })
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 6);

        // Check await patterns
        assert_eq!(parsed[0]["await"], true);
        assert_eq!(parsed[1]["await"], false);
        assert_eq!(parsed[2]["await"], false);
        assert_eq!(parsed[3]["await"], true);
        assert_eq!(parsed[4]["await"], false);
        assert_eq!(parsed[5]["await"], true);
    }

    #[tokio::test]
    async fn test_workflow_variables_simple() {
        let source = r#"workflow(ctx, inputs) {
    let user_id = await task("create_user", { "name": "Alice" })
    await task("send_email", { "user_id": user_id, "subject": "Welcome" })
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 2);

        // First task: variable assignment
        assert_eq!(parsed[0]["type"], "task");
        assert_eq!(parsed[0]["task"], "create_user");
        assert_eq!(parsed[0]["assign_to"], "user_id");
        assert_eq!(parsed[0]["await"], true);

        // Second task: uses variable
        assert_eq!(parsed[1]["type"], "task");
        assert_eq!(parsed[1]["task"], "send_email");
        assert_eq!(parsed[1]["inputs"]["user_id"], "$user_id", "Variable reference uses JSON object format");
        assert_eq!(parsed[1]["inputs"]["subject"], "Welcome");
    }

    #[tokio::test]
    async fn test_workflow_variables_multiple() {
        let source = r#"workflow(ctx, inputs) {
    let order_id = await task("create_order", { "amount": 100, "currency": "USD" })
    let payment_id = await task("process_payment", { "order_id": order_id, "method": "card" })
    let receipt_id = await task("generate_receipt", { "order_id": order_id, "payment_id": payment_id })
    await task("send_confirmation", { "order": order_id, "payment": payment_id, "receipt": receipt_id })
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 4);

        // Check variable assignments
        assert_eq!(parsed[0]["assign_to"], "order_id");
        assert_eq!(parsed[1]["assign_to"], "payment_id");
        assert_eq!(parsed[2]["assign_to"], "receipt_id");

        // Check variable usage in second task
        assert_eq!(parsed[1]["inputs"]["order_id"], "$order_id");

        // Check multiple variables in third task
        assert_eq!(parsed[2]["inputs"]["order_id"], "$order_id");
        assert_eq!(parsed[2]["inputs"]["payment_id"], "$payment_id");

        // Check all three variables in final task
        assert_eq!(parsed[3]["inputs"]["order"], "$order_id");
        assert_eq!(parsed[3]["inputs"]["payment"], "$payment_id");
        assert_eq!(parsed[3]["inputs"]["receipt"], "$receipt_id");
    }

    #[tokio::test]
    async fn test_workflow_variables_fire_and_forget() {
        let source = r#"workflow(ctx, inputs) {
    let result1 = task("background_job1", { "priority": "low" })
    let result2 = task("background_job2", { "priority": "low" })
    await task("final_task", { "msg": "done" })
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 3);

        // Fire-and-forget with assignment
        assert_eq!(parsed[0]["assign_to"], "result1");
        assert_eq!(parsed[0]["await"], false);

        assert_eq!(parsed[1]["assign_to"], "result2");
        assert_eq!(parsed[1]["await"], false);

        assert_eq!(parsed[2]["await"], true);
    }

    #[tokio::test]
    async fn test_workflow_json_all_types() {
        let source = r#"workflow(ctx, inputs) {
    await task("test_types", {
        "string_val": "hello world",
        "number_int": 42,
        "number_float": 3.14159,
        "bool_true": true,
        "bool_false": false,
        "null_val": null,
        "array_val": [1, 2, "three", true, null],
        "object_val": {
            "nested": "value",
            "count": 10
        }
    })
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 1);

        let inputs = &parsed[0]["inputs"];

        // Test all JSON types
        assert_eq!(inputs["string_val"], "hello world");
        assert_eq!(inputs["number_int"], 42);
        assert_eq!(inputs["number_float"], 3.14159);
        assert_eq!(inputs["bool_true"], true);
        assert_eq!(inputs["bool_false"], false);
        assert_eq!(inputs["null_val"], serde_json::Value::Null);

        // Test array
        let array = &inputs["array_val"];
        assert!(array.is_array());
        assert_eq!(array[0], 1);
        assert_eq!(array[1], 2);
        assert_eq!(array[2], "three");
        assert_eq!(array[3], true);
        assert_eq!(array[4], serde_json::Value::Null);

        // Test nested object
        let obj = &inputs["object_val"];
        assert!(obj.is_object());
        assert_eq!(obj["nested"], "value");
        assert_eq!(obj["count"], 10);
    }

    #[tokio::test]
    async fn test_workflow_variables_in_complex_json() {
        let source = r#"workflow(ctx, inputs) {
    let user_id = await task("get_user", { "email": "test@example.com" })
    let config = await task("get_config", { "env": "production" })

    await task("process", {
        "users": [user_id, "user2", "user3"],
        "settings": {
            "primary_user": user_id,
            "config": config,
            "metadata": {
                "created_by": user_id
            }
        },
        "mixed": [1, user_id, { "nested": config }, true]
    })
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 3);

        let inputs = &parsed[2]["inputs"];

        // Check variable in array
        assert_eq!(inputs["users"][0], "$user_id");
        assert_eq!(inputs["users"][1], "user2");
        assert_eq!(inputs["users"][2], "user3");

        // Check variables in nested objects
        assert_eq!(inputs["settings"]["primary_user"], "$user_id");
        assert_eq!(inputs["settings"]["config"], "$config");
        assert_eq!(inputs["settings"]["metadata"]["created_by"], "$user_id");

        // Check variables in mixed array with objects
        assert_eq!(inputs["mixed"][0], 1);
        assert_eq!(inputs["mixed"][1], "$user_id");
        assert_eq!(inputs["mixed"][2]["nested"], "$config");
        assert_eq!(inputs["mixed"][3], true);
    }

    #[tokio::test]
    async fn test_workflow_comments_and_whitespace() {
        let source = r#"workflow(ctx, inputs) {
    // This is a comment
    await task("task1", { "step": 1 })

    // Another comment
    // Multiple comment lines

    await task("task2", { "step": 2 })
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 2, "Comments should be ignored");
        assert_eq!(parsed[0]["task"], "task1");
        assert_eq!(parsed[1]["task"], "task2");
    }

    #[tokio::test]
    async fn test_workflow_single_quotes() {
        let source = r#"workflow(ctx, inputs) {
    await task('task_with_single_quotes', { 'key': 'value' })
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["task"], "task_with_single_quotes");
        assert_eq!(parsed[0]["inputs"]["key"], "value");
    }

    #[tokio::test]
    async fn test_workflow_variable_naming_conventions() {
        let source = r#"workflow(ctx, inputs) {
    let snake_case_var = await task("test1", {})
    let camelCaseVar = await task("test2", {})
    let _private_var = await task("test3", {})
    let var123 = await task("test4", {})
    let _123mixed = await task("test5", {})
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 5);

        assert_eq!(parsed[0]["assign_to"], "snake_case_var");
        assert_eq!(parsed[1]["assign_to"], "camelCaseVar");
        assert_eq!(parsed[2]["assign_to"], "_private_var");
        assert_eq!(parsed[3]["assign_to"], "var123");
        assert_eq!(parsed[4]["assign_to"], "_123mixed");
    }
}
