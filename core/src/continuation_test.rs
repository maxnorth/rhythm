/// Integration tests for workflow continuation mechanism
#[cfg(test)]
mod tests {
    use crate::db::get_pool;
    use crate::db::test_helpers::with_test_db;
    use crate::executions::complete_execution;
    use crate::interpreter::executor::{execute_workflow_step, StepResult};
    use crate::workflows::{register_workflows, start_workflow, WorkflowFile};
    use anyhow::Result;
    use serde_json::json;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_task_completion_enqueues_resume_task() -> Result<()> {
        let _guard = with_test_db().await;

        // Load and register test workflow
        let workflow_source = std::fs::read_to_string("test_workflows/test_continuation.flow")?;
        let workflows = vec![WorkflowFile {
            name: "test_continuation".to_string(),
            source: workflow_source,
            file_path: "test_workflows/test_continuation.flow".to_string(),
        }];
        register_workflows(workflows).await?;

        // Start workflow
        let workflow_id = start_workflow("test_continuation", json!({"test": "data"})).await?;

        println!("Created workflow: {}", workflow_id);

        // Execute first step - should suspend waiting for task
        let result = execute_workflow_step(&workflow_id).await?;
        println!("Step 1 result: {:?}", result);

        // Workflow should be suspended
        assert!(matches!(result, StepResult::Suspended));

        // Find the child task that was created
        let pool = get_pool().await?;
        let task_id: Option<String> = sqlx::query_scalar(
            r#"
            SELECT id FROM executions
            WHERE parent_workflow_id = $1
            AND function_name = 'process_data'
            AND status = 'pending'
            "#,
        )
        .bind(&workflow_id)
        .fetch_optional(pool.as_ref())
        .await?;

        let task_id = task_id.expect("Task should have been created");
        println!("Found child task: {}", task_id);

        // Complete the task - this should enqueue a resume task
        complete_execution(&task_id, json!({"value": 42})).await?;
        println!("Completed task");

        // Verify resume task was created
        let resume_task_id: Option<String> = sqlx::query_scalar(
            r#"
            SELECT id FROM executions
            WHERE function_name = 'builtin.resume_workflow'
            AND args::jsonb @> $1::jsonb
            AND status = 'pending'
            "#,
        )
        .bind(json!([workflow_id]))
        .fetch_optional(pool.as_ref())
        .await?;

        let resume_task_id = resume_task_id.expect("Resume task should have been created");
        println!("Found resume task: {}", resume_task_id);

        // Execute the resume task (simulating worker processing it)
        // This should call execute_workflow_step on the workflow
        let result = execute_workflow_step(&workflow_id).await?;
        println!("Resume result: {:?}", result);

        // Workflow should now be completed
        let workflow_status: String = sqlx::query_scalar(
            "SELECT status::text FROM executions WHERE id = $1"
        )
        .bind(&workflow_id)
        .fetch_one(pool.as_ref())
        .await?;

        println!("Final workflow status: {}", workflow_status);
        assert_eq!(workflow_status, "completed");

        println!("✅ Workflow continuation test passed!");

        Ok(())
    }
}
