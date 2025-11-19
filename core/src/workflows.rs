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
        // 1. Parse workflow source to JSON AST
        let steps = interpreter::parse_workflow(&workflow.source)
            .with_context(|| format!("Failed to parse workflow '{}' from {}", workflow.name, workflow.file_path))?;

        // 2. Validate the AST (semantic analysis)
        let steps_json = serde_json::to_value(&steps)
            .context("Failed to convert steps to JSON")?;
        interpreter::validate_workflow(&steps_json)
            .with_context(|| format!("Validation failed for workflow '{}' from {}", workflow.name, workflow.file_path))?;

        // Hash the source to create version
        let version_hash = hash_source(&workflow.source);

        // Serialize parsed steps to JSON
        let parsed_steps = serde_json::to_value(&steps)
            .context("Failed to serialize parsed steps")?;

        println!(
            "✓ Registered workflow '{}' (version: {}, {} steps)",
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

    let exec_result = sqlx::query(
        r#"
        INSERT INTO executions (
            id, type, function_name, queue, status,
            inputs
        ) VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(&execution_id)
    .bind(&ExecutionType::Workflow)
    .bind(workflow_name)
    .bind("default") // Use default queue
    .bind(&ExecutionStatus::Pending)
    .bind(&inputs)
    .execute(pool.as_ref())
    .await;

    if let Err(e) = exec_result {
        return Err(anyhow::anyhow!("Failed to create workflow execution (INSERT into executions): {}", e));
    }

    // 3. Create workflow execution context with initial state
    sqlx::query(
        r#"
        INSERT INTO workflow_execution_context (
            execution_id, workflow_definition_id,
            ast_path, locals
        ) VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(&execution_id)
    .bind(workflow_def_id)
    .bind(serde_json::json!([0])) // Start at path [0]
    .bind(serde_json::json!({})) // Empty locals initially
    .execute(pool.as_ref())
    .await
    .context("Failed to create workflow execution context")?;

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

    #[tokio::test(flavor = "multi_thread")]
    async fn test_register_empty_workflows() {
        let result = register_workflows(vec![]).await;
        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_register_workflow_parsing() {
        let source = r#"workflow(ctx, inputs) {
    Task.run("myTask", { "input": "value" })
}"#;

        // Just test that parsing works
        let result = parse_workflow(source);
        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_register_workflow_parse_error() {
        let source = "invalid syntax here";

        // Just test that parsing fails
        let result = parse_workflow(source);
        assert!(result.is_err());
    }

    // E2E Tests for various workflow syntax patterns

    #[tokio::test(flavor = "multi_thread")]
    async fn test_workflow_basic_sequential() {
        let source = r#"workflow(ctx, inputs) {
    await Task.run("task1", { "step": 1 })
    await Task.run("task2", { "step": 2 })
    await Task.run("task3", { "step": 3 })
}"#;

        // Just test parsing, not registration (which requires DB)
        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 3, "Expected 3 statements");

        for (i, step) in parsed.iter().enumerate() {
            assert_eq!(step["type"], "await");
            assert_eq!(step["expression"]["type"], "function_call");
            assert_eq!(step["expression"]["name"], json!(["Task", "run"]));
            assert_eq!(step["expression"]["args"][1]["step"], i + 1);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_workflow_fire_and_forget() {
        let source = r#"workflow(ctx, inputs) {
    Task.run("task1", { "async": true })
    Task.run("task2", { "async": true })
    Task.run("task3", { "async": true })
}"#;

        // Just test parsing, not registration (which requires DB)
        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 3);

        for step in parsed.iter() {
            assert_eq!(step["type"], "expression_statement");
            assert_eq!(step["expression"]["type"], "function_call");
            assert_eq!(step["expression"]["name"], json!(["Task", "run"]));
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_workflow_mixed_await() {
        let source = r#"workflow(ctx, inputs) {
    await Task.run("init", { "step": 1 })
    Task.run("background1", { "async": true })
    Task.run("background2", { "async": true })
    await Task.run("checkpoint", { "step": 2 })
    Task.run("background3", { "async": true })
    await Task.run("finalize", { "step": 3 })
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 6);

        // Check types (await vs expression_statement)
        assert_eq!(parsed[0]["type"], "await");
        assert_eq!(parsed[1]["type"], "expression_statement");
        assert_eq!(parsed[2]["type"], "expression_statement");
        assert_eq!(parsed[3]["type"], "await");
        assert_eq!(parsed[4]["type"], "expression_statement");
        assert_eq!(parsed[5]["type"], "await");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_workflow_variables_simple() {
        let source = r#"workflow(ctx, inputs) {
    let user_id = await Task.run("create_user", { "name": "Alice" })
    await Task.run("send_email", { "user_id": user_id, "subject": "Welcome" })
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 2);

        // First task: variable assignment
        assert_eq!(parsed[0]["type"], "assignment");
        assert_eq!(parsed[0]["left"]["type"], "variable");
        assert_eq!(parsed[0]["left"]["name"], "user_id");
        assert_eq!(parsed[0]["right"]["type"], "await");
        assert_eq!(parsed[0]["right"]["expression"]["type"], "function_call");
        assert_eq!(parsed[0]["right"]["expression"]["name"], json!(["Task", "run"]));
        assert_eq!(parsed[0]["right"]["expression"]["args"][0], "create_user");

        // Second task: uses variable (new format: {"type": "variable", "name": "name", "depth": 0})
        assert_eq!(parsed[1]["type"], "await");
        assert_eq!(parsed[1]["expression"]["args"][0], "send_email");
        assert_eq!(parsed[1]["expression"]["args"][1]["user_id"]["type"], "variable");
        assert_eq!(parsed[1]["expression"]["args"][1]["user_id"]["name"], "user_id");
        assert_eq!(parsed[1]["expression"]["args"][1]["user_id"]["depth"], 0);
        assert_eq!(parsed[1]["expression"]["args"][1]["subject"], "Welcome");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_workflow_variables_multiple() {
        let source = r#"workflow(ctx, inputs) {
    let order_id = await Task.run("create_order", { "amount": 100, "currency": "USD" })
    let payment_id = await Task.run("process_payment", { "order_id": order_id, "method": "card" })
    let receipt_id = await Task.run("generate_receipt", { "order_id": order_id, "payment_id": payment_id })
    await Task.run("send_confirmation", { "order": order_id, "payment": payment_id, "receipt": receipt_id })
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 4);

        // Check variable assignments
        assert_eq!(parsed[0]["left"]["name"], "order_id");
        assert_eq!(parsed[1]["left"]["name"], "payment_id");
        assert_eq!(parsed[2]["left"]["name"], "receipt_id");

        // Check variable usage in second task (new format)
        assert_eq!(parsed[1]["right"]["expression"]["args"][1]["order_id"]["type"], "variable");
        assert_eq!(parsed[1]["right"]["expression"]["args"][1]["order_id"]["name"], "order_id");
        assert_eq!(parsed[1]["right"]["expression"]["args"][1]["order_id"]["depth"], 0);

        // Check multiple variables in third task
        assert_eq!(parsed[2]["right"]["expression"]["args"][1]["order_id"]["type"], "variable");
        assert_eq!(parsed[2]["right"]["expression"]["args"][1]["payment_id"]["type"], "variable");

        // Check all three variables in final task
        assert_eq!(parsed[3]["expression"]["args"][1]["order"]["type"], "variable");
        assert_eq!(parsed[3]["expression"]["args"][1]["payment"]["type"], "variable");
        assert_eq!(parsed[3]["expression"]["args"][1]["receipt"]["type"], "variable");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_workflow_variables_fire_and_forget() {
        let source = r#"workflow(ctx, inputs) {
    let result1 = Task.run("background_job1", { "priority": "low" })
    let result2 = Task.run("background_job2", { "priority": "low" })
    await Task.run("final_task", { "msg": "done" })
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 3);

        // Fire-and-forget with assignment
        assert_eq!(parsed[0]["type"], "assignment");
        assert_eq!(parsed[0]["left"]["name"], "result1");
        assert_eq!(parsed[0]["right"]["type"], "function_call");  // No await, just function call

        assert_eq!(parsed[1]["type"], "assignment");
        assert_eq!(parsed[1]["left"]["name"], "result2");
        assert_eq!(parsed[1]["right"]["type"], "function_call");

        assert_eq!(parsed[2]["type"], "await");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_workflow_json_all_types() {
        let source = r#"workflow(ctx, inputs) {
    await Task.run("test_types", {
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

        let inputs = &parsed[0]["expression"]["args"][1];

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

    #[tokio::test(flavor = "multi_thread")]
    async fn test_workflow_variables_in_complex_json() {
        let source = r#"workflow(ctx, inputs) {
    let user_id = await Task.run("get_user", { "email": "test@example.com" })
    let config = await Task.run("get_config", { "env": "production" })

    await Task.run("process", {
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

        let inputs = &parsed[2]["expression"]["args"][1];

        // Check variable in array (new format with type field)
        assert_eq!(inputs["users"][0]["type"], "variable");
        assert_eq!(inputs["users"][0]["name"], "user_id");
        assert_eq!(inputs["users"][1], "user2");
        assert_eq!(inputs["users"][2], "user3");

        // Check variables in nested objects
        assert_eq!(inputs["settings"]["primary_user"]["type"], "variable");
        assert_eq!(inputs["settings"]["primary_user"]["name"], "user_id");
        assert_eq!(inputs["settings"]["config"]["type"], "variable");
        assert_eq!(inputs["settings"]["config"]["name"], "config");
        assert_eq!(inputs["settings"]["metadata"]["created_by"]["type"], "variable");
        assert_eq!(inputs["settings"]["metadata"]["created_by"]["name"], "user_id");

        // Check variables in mixed array with objects
        assert_eq!(inputs["mixed"][0], 1);
        assert_eq!(inputs["mixed"][1]["type"], "variable");
        assert_eq!(inputs["mixed"][1]["name"], "user_id");
        assert_eq!(inputs["mixed"][2]["nested"]["type"], "variable");
        assert_eq!(inputs["mixed"][2]["nested"]["name"], "config");
        assert_eq!(inputs["mixed"][3], true);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_workflow_comments_and_whitespace() {
        let source = r#"workflow(ctx, inputs) {
    // This is a comment
    await Task.run("task1", { "step": 1 })

    // Another comment
    // Multiple comment lines

    await Task.run("task2", { "step": 2 })
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 2, "Comments should be ignored");
        assert_eq!(parsed[0]["expression"]["args"][0], "task1");
        assert_eq!(parsed[1]["expression"]["args"][0], "task2");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_workflow_single_quotes() {
        let source = r#"workflow(ctx, inputs) {
    await Task.run('task_with_single_quotes', { 'key': 'value' })
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["expression"]["args"][0], "task_with_single_quotes");
        assert_eq!(parsed[0]["expression"]["args"][1]["key"], "value");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_workflow_variable_naming_conventions() {
        let source = r#"workflow(ctx, inputs) {
    let snake_case_var = await Task.run("test1", {})
    let camelCaseVar = await Task.run("test2", {})
    let _private_var = await Task.run("test3", {})
    let var123 = await Task.run("test4", {})
    let _123mixed = await Task.run("test5", {})
}"#;

        let parsed = parse_workflow(source).unwrap();
        assert_eq!(parsed.len(), 5);

        assert_eq!(parsed[0]["left"]["name"], "snake_case_var");
        assert_eq!(parsed[1]["left"]["name"], "camelCaseVar");
        assert_eq!(parsed[2]["left"]["name"], "_private_var");
        assert_eq!(parsed[3]["left"]["name"], "var123");
        assert_eq!(parsed[4]["left"]["name"], "_123mixed");
    }

    // === SEMANTIC VALIDATION TESTS ===

    #[test]
    fn test_semantic_validation_task_run_missing_args() {
        use crate::interpreter::{parse_workflow, validate_workflow};

        let source = r#"
workflow(ctx, inputs) {
    Task.run()
}
        "#;
        let ast = parse_workflow(source).unwrap();
        let ast_json = serde_json::to_value(&ast).unwrap();

        let result = validate_workflow(&ast_json);
        assert!(result.is_err(), "Expected validation to fail for Task.run with no arguments");
        let err = result.unwrap_err();
        assert!(err.to_string().contains("expects 1-2 arguments"), "Error should mention argument count");
    }

    #[test]
    fn test_semantic_validation_break_outside_loop() {
        use crate::interpreter::{parse_workflow, validate_workflow};

        let source = r#"
workflow(ctx, inputs) {
    break
}
        "#;
        let ast = parse_workflow(source).unwrap();
        let ast_json = serde_json::to_value(&ast).unwrap();

        let result = validate_workflow(&ast_json);
        assert!(result.is_err(), "Expected validation to fail for break outside loop");
        let err = result.unwrap_err();
        assert!(err.to_string().contains("outside of loop"), "Error should mention loop requirement");
    }

    #[test]
    fn test_semantic_validation_undefined_variable() {
        use crate::interpreter::{parse_workflow, validate_workflow};

        let source = r#"
workflow(ctx, inputs) {
    await Task.run("task1", { value: undefined_var })
}
        "#;
        let ast = parse_workflow(source).unwrap();
        let ast_json = serde_json::to_value(&ast).unwrap();

        let result = validate_workflow(&ast_json);
        assert!(result.is_err(), "Expected validation to fail for undefined variable");
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Undefined variable"), "Error should mention undefined variable");
    }

    #[test]
    fn test_semantic_validation_valid_workflow() {
        use crate::interpreter::{parse_workflow, validate_workflow};

        let source = r#"
workflow(ctx, inputs) {
    let result = await Task.run("my-task", {})
    await Task.run("another-task", {})
}
        "#;
        let ast = parse_workflow(source).unwrap();
        let ast_json = serde_json::to_value(&ast).unwrap();

        let result = validate_workflow(&ast_json);
        assert!(result.is_ok(), "Expected validation to pass for valid workflow: {:?}", result.err());
    }
}
