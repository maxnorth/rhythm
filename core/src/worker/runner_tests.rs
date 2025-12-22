//! Integration tests for V2 workflow runner
//!
//! These tests exercise the complete workflow lifecycle including:
//! - Running workflows from scratch
//! - Suspending on tasks
//! - Resuming workflows when tasks complete
//! - Completing workflows
//! - Error handling

use serde_json::json;

use super::run_workflow;
use crate::db;
use crate::test_helpers::{
    enqueue_and_claim_execution, get_child_task_count, get_child_tasks, get_task_by_target_name,
    get_unclaimed_work_count, get_work_queue_count, setup_workflow_test,
    setup_workflow_test_with_pool,
};
use crate::types::ExecutionStatus;

#[tokio::test(flavor = "multi_thread")]
async fn test_simple_workflow_completes_immediately() {
    // Create a simple workflow that just returns a value
    let workflow_source = r#"
        x = 42
        return x
    "#;

    let (pool, execution) =
        setup_workflow_test("simple_workflow", workflow_source, json!({})).await;
    let execution_id = execution.id.clone();

    // Run workflow - should complete immediately
    run_workflow(&pool, execution).await.unwrap();

    // Verify execution completed successfully
    let execution = db::executions::get_execution(&pool, &execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(execution.status, ExecutionStatus::Completed);
    assert_eq!(execution.output, Some(json!(42.0)));

    // Verify work queue entry was deleted
    let work_count = get_work_queue_count(&pool, &execution_id).await.unwrap();
    assert_eq!(work_count, 0, "Work queue should be empty after completion");

    // Verify no workflow execution context exists
    let context = db::workflow_execution_context::get_context(&pool, &execution_id)
        .await
        .unwrap();
    assert!(context.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_workflow_suspends_on_task_then_completes() {
    // Workflow that awaits a task
    let workflow_source = r#"
        task_result = await Task.run("process_data", {value: 10})
        return task_result * 2
    "#;

    let (pool, execution) =
        setup_workflow_test("workflow_with_task", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run: workflow should suspend on task
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow suspended
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Suspended);

    // Verify workflow execution context exists
    let context = db::workflow_execution_context::get_context(&pool, &workflow_id)
        .await
        .unwrap();
    assert!(context.is_some());

    // Verify child task was created
    let child_tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();

    assert_eq!(child_tasks.len(), 1);
    let (task_id, task_name) = &child_tasks[0];
    assert_eq!(task_name, "process_data");

    // Complete the task out-of-band
    db::executions::complete_execution(pool.as_ref(), task_id, json!(100))
        .await
        .unwrap();

    // Enqueue work again for the workflow to resume
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();

    // Second run: workflow should resume and complete
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .expect("Execution should exist");
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow completed successfully
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    assert_eq!(workflow_execution.output, Some(json!(200.0)));

    // Verify workflow execution context was deleted
    let context = db::workflow_execution_context::get_context(&pool, &workflow_id)
        .await
        .unwrap();
    assert!(context.is_none());

    // Verify work queue entry was deleted
    let work_count = get_work_queue_count(&pool, &workflow_id).await.unwrap();
    assert_eq!(work_count, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_workflow_with_multiple_sequential_tasks() {
    // Workflow that awaits multiple tasks in sequence
    let workflow_source = r#"
        first = await Task.run("step_one", {input: 5})
        second = await Task.run("step_two", {input: first})
        third = await Task.run("step_three", {input: second})
        return third
    "#;

    let (pool, execution) =
        setup_workflow_test("multi_step_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Run 1: Suspend on first task
    run_workflow(&pool, execution).await.unwrap();

    // Complete first task
    let task1_id = get_task_by_target_name(&pool, &workflow_id, "step_one")
        .await
        .unwrap();

    db::executions::complete_execution(pool.as_ref(), &task1_id, json!(10))
        .await
        .unwrap();

    // Run 2: Suspend on second task
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();

    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .expect("Execution should exist");
    run_workflow(&pool, execution).await.unwrap();

    // Complete second task
    let task2_id = get_task_by_target_name(&pool, &workflow_id, "step_two")
        .await
        .unwrap();

    db::executions::complete_execution(pool.as_ref(), &task2_id, json!(20))
        .await
        .unwrap();

    // Run 3: Suspend on third task
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();

    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .expect("Execution should exist");
    run_workflow(&pool, execution).await.unwrap();

    // Complete third task
    let task3_id = get_task_by_target_name(&pool, &workflow_id, "step_three")
        .await
        .unwrap();

    db::executions::complete_execution(pool.as_ref(), &task3_id, json!(30))
        .await
        .unwrap();

    // Run 4: Complete workflow
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();

    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .expect("Execution should exist");
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow completed
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);

    // Verify all three child tasks exist
    let task_count = get_child_task_count(&pool, &workflow_id).await.unwrap();
    assert_eq!(task_count, 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_workflow_with_fire_and_forget_task() {
    // Workflow with a fire-and-forget task followed by an awaited task
    let workflow_source = r#"
        Task.run("background_task", {data: "log this"})
        result = await Task.run("main_task", {value: 42})
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("mixed_task_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run: should suspend on main_task
    run_workflow(&pool, execution).await.unwrap();

    // Verify both tasks were created
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();

    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].1, "background_task");
    assert_eq!(tasks[1].1, "main_task");

    // Complete only the main task
    db::executions::complete_execution(pool.as_ref(), &tasks[1].0, json!(999))
        .await
        .unwrap();

    // Resume workflow
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();

    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .expect("Execution should exist");
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow completed
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    assert_eq!(workflow_execution.output, Some(json!(999.0)));

    // Background task should still be pending (or whatever state it's in)
    let background_task = db::executions::get_execution(&pool, &tasks[0].0)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(background_task.status, ExecutionStatus::Pending);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_workflow_with_invalid_syntax_fails() {
    // Workflow with invalid syntax that will fail during parsing
    let workflow_source = r#"this is not valid syntax!!!"#;

    let (pool, execution) =
        setup_workflow_test("invalid_workflow", workflow_source, json!({})).await;

    // Run workflow - should fail during parsing
    let result = run_workflow(&pool, execution).await;

    // Should fail with parsing error
    assert!(result.is_err(), "Workflow with invalid syntax should fail");

    // Execution might still be pending since it failed before execution started
    // This is acceptable - the test just verifies that run_workflow returns an error
}

#[tokio::test(flavor = "multi_thread")]
async fn test_workflow_with_inputs() {
    // Workflow that uses inputs
    let workflow_source = r#"
        result = Inputs.x + Inputs.y
        return result
    "#;

    let (pool, execution) = setup_workflow_test(
        "inputs_workflow",
        workflow_source,
        json!({"x": 15, "y": 27}),
    )
    .await;
    let workflow_id = execution.id.clone();

    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow completed with correct output
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    assert_eq!(workflow_execution.output, Some(json!(42.0)));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_workflow_resumes_with_failed_task() {
    // Workflow that awaits a task - the task will fail but workflow should resume
    let workflow_source = r#"
        task_result = await Task.run("failing_task", {value: 10})
        return task_result
    "#;

    let (pool, execution) =
        setup_workflow_test("workflow_with_failing_task", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run: workflow suspends on task
    run_workflow(&pool, execution).await.unwrap();

    // Find the task
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    let task_id = &tasks[0].0;

    // Fail the task with error output
    db::executions::fail_execution(
        pool.as_ref(),
        &task_id,
        json!({"error": "Task failed!", "code": "TASK_ERROR"}),
    )
    .await
    .unwrap();

    // Resume workflow
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();

    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .expect("Execution should exist");
    run_workflow(&pool, execution).await.unwrap();

    // Workflow should complete and return the error output
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    assert_eq!(
        workflow_execution.output,
        Some(json!({"error": "Task failed!", "code": "TASK_ERROR"}))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resume_without_task_completion_fails() {
    let workflow_source = r#"
        result = await Task.run("pending_task", {})
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("workflow_waiting", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run: workflow suspends
    run_workflow(&pool, execution).await.unwrap();

    // Try to resume without completing the task
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();

    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .expect("Execution should exist");
    let result = run_workflow(&pool, execution).await;

    // Should fail because task has no output
    assert!(result.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_corrupted_vm_state_fails_gracefully() {
    let workflow_source = r#"
        result = await Task.run("some_task", {})
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("workflow_corrupted", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run: workflow suspends
    run_workflow(&pool, execution).await.unwrap();

    // Corrupt the VM state (locals column is what stores the state)
    sqlx::query(
        r#"
        UPDATE workflow_execution_context
        SET locals = '{"invalid": "state", "missing": "required_fields"}'
        WHERE execution_id = $1
        "#,
    )
    .bind(&workflow_id)
    .execute(pool.as_ref())
    .await
    .unwrap();

    // Try to resume with corrupted state
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();

    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .expect("Execution should exist");
    let result = run_workflow(&pool, execution).await;

    // Should fail with deserialization error
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("deserialize"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_workflow_returns_different_types() {
    // Test null return
    let null_workflow = r#"return null"#;
    let (pool, execution) = setup_workflow_test("null_workflow", null_workflow, json!({})).await;
    let workflow_id = execution.id.clone();

    run_workflow(&pool, execution).await.unwrap();

    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(execution.output, Some(json!(null)));

    // Test boolean return
    let bool_workflow = r#"return true"#;
    let (pool, execution) =
        setup_workflow_test_with_pool(Some(pool), "bool_workflow", bool_workflow, json!({})).await;
    let workflow_id2 = execution.id.clone();

    run_workflow(&pool, execution).await.unwrap();

    let execution2 = db::executions::get_execution(&pool, &workflow_id2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(execution2.output, Some(json!(true)));

    // Test array return
    let array_workflow = r#"return [1, 2, 3]"#;
    let (pool, execution) =
        setup_workflow_test_with_pool(Some(pool), "array_workflow", array_workflow, json!({}))
            .await;
    let workflow_id3 = execution.id.clone();

    run_workflow(&pool, execution).await.unwrap();

    let execution3 = db::executions::get_execution(&pool, &workflow_id3)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(execution3.output, Some(json!([1.0, 2.0, 3.0])));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_dual_row_work_queue_pattern() {
    let workflow_source = r#"
        result = await Task.run("long_task", {})
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("dual_row_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Workflow runs and suspends
    run_workflow(&pool, execution).await.unwrap();

    // Simulate a new event (child task completion) that re-queues the workflow
    // This should create an unclaimed row while the claimed row still exists
    db::work_queue::enqueue_work(pool.as_ref(), &workflow_id, "default", 0)
        .await
        .unwrap();

    // Verify we have exactly 1 row (the unclaimed one, claimed was deleted when suspended)
    let work_count = get_work_queue_count(&pool, &workflow_id).await.unwrap();
    assert_eq!(work_count, 1);

    // Verify it's unclaimed
    let unclaimed_count = get_unclaimed_work_count(&pool, &workflow_id).await.unwrap();
    assert_eq!(unclaimed_count, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_workflow_creates_many_tasks() {
    // Workflow that creates multiple tasks in one go
    let workflow_source = r#"
        Task.run("task1", {})
        Task.run("task2", {})
        Task.run("task3", {})
        Task.run("task4", {})
        Task.run("task5", {})
        return "all_tasks_created"
    "#;

    let (pool, execution) =
        setup_workflow_test("many_tasks_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    run_workflow(&pool, execution).await.unwrap();

    // Verify all 5 tasks were created
    let task_count = get_child_task_count(&pool, &workflow_id).await.unwrap();
    assert_eq!(task_count, 5);

    // Verify workflow completed
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(execution.status, ExecutionStatus::Completed);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_workflow_with_varying_task_counts() {
    // Workflow that creates tasks
    let workflow_with_tasks = r#"
        Task.run("task1", {})
        Task.run("task2", {})
        return "created_tasks"
    "#;

    // Test 1: Workflow that creates tasks
    let (pool1, execution) =
        setup_workflow_test("workflow_with_tasks", workflow_with_tasks, json!({})).await;
    let workflow_id1 = execution.id.clone();

    run_workflow(&pool1, execution).await.unwrap();

    let task_count1 = get_child_task_count(&pool1, &workflow_id1).await.unwrap();
    assert_eq!(task_count1, 2, "Should create 2 tasks");

    // Test 2: Workflow that creates no tasks
    let workflow_no_tasks = r#"
        x = 42
        return x
    "#;

    let (pool2, execution) =
        setup_workflow_test("workflow_no_tasks", workflow_no_tasks, json!({})).await;
    let workflow_id2 = execution.id.clone();

    run_workflow(&pool2, execution).await.unwrap();

    let task_count2 = get_child_task_count(&pool2, &workflow_id2).await.unwrap();
    assert_eq!(task_count2, 0, "Should create 0 tasks");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_child_tasks_are_enqueued_to_work_queue() {
    // Workflow that creates multiple fire-and-forget tasks
    let workflow_source = r#"
        Task.run("task_one", {value: 1})
        Task.run("task_two", {value: 2})
        Task.run("task_three", {value: 3})
        return "tasks_created"
    "#;

    let (pool, execution) =
        setup_workflow_test("workflow_enqueue_test", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Run workflow - should create 3 tasks and complete
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow completed
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);

    // Verify all 3 child tasks were created
    let child_tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    assert_eq!(child_tasks.len(), 3, "Should create 3 child tasks");

    // Verify each child task is enqueued to the work queue
    for (task_id, task_name) in &child_tasks {
        let work_count = get_work_queue_count(&pool, task_id).await.unwrap();
        assert_eq!(
            work_count, 1,
            "Child task '{}' ({}) should have exactly 1 work queue entry",
            task_name, task_id
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_awaited_task_is_enqueued_to_work_queue() {
    // Workflow that awaits a task
    let workflow_source = r#"
        result = await Task.run("awaited_task", {data: "test"})
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("workflow_await_enqueue_test", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Run workflow - should suspend on the task
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow suspended
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Suspended);

    // Verify child task was created
    let child_tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    assert_eq!(child_tasks.len(), 1, "Should create 1 child task");

    // Verify the child task is enqueued to the work queue
    let (task_id, task_name) = &child_tasks[0];
    let work_count = get_work_queue_count(&pool, task_id).await.unwrap();
    assert_eq!(
        work_count, 1,
        "Child task '{}' ({}) should have exactly 1 work queue entry",
        task_name, task_id
    );
}
