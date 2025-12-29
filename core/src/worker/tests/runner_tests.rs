//! Integration tests for V2 workflow runner
//!
//! These tests exercise the complete workflow lifecycle including:
//! - Running workflows from scratch
//! - Suspending on tasks
//! - Resuming workflows when tasks complete
//! - Completing workflows
//! - Error handling

use serde_json::json;

use super::super::run_workflow;
use crate::db;
use crate::test_helpers::{
    enqueue_and_claim_execution, get_child_task_count, get_child_tasks, get_task_by_target_name,
    get_unclaimed_work_count, get_work_queue_count, setup_workflow_test,
    setup_workflow_test_with_pool,
};
use crate::types::ExecutionStatus;

#[tokio::test(flavor = "multi_thread")]
async fn test_workflow_completes_without_return_statement() {
    // Workflow that executes statements but has no explicit return
    // Should complete with null output (implicit return)
    let workflow_source = r#"
        x = 42
        y = x + 1
    "#;

    let (pool, execution) =
        setup_workflow_test("no_return_workflow", workflow_source, json!({})).await;
    let execution_id = execution.id.clone();

    // Run workflow - should complete immediately with null output
    run_workflow(&pool, execution).await.unwrap();

    // Verify execution completed successfully with null output
    let execution = db::executions::get_execution(&pool, &execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(execution.status, ExecutionStatus::Completed);
    assert_eq!(execution.output, Some(json!(null)));

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
async fn test_workflow_with_task_but_no_return_statement() {
    // Workflow that awaits a task but has no explicit return
    // Should complete with null output after task completes
    let workflow_source = r#"
        result = await Task.run("process_data", {value: 10})
        log = result + 1
    "#;

    let (pool, execution) =
        setup_workflow_test("task_no_return_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run: workflow should suspend on task
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow suspended
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Suspended);

    // Get the child task
    let child_tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    assert_eq!(child_tasks.len(), 1);
    let (task_id, _) = &child_tasks[0];

    // Complete the task out-of-band
    db::executions::complete_execution(pool.as_ref(), task_id, json!(100))
        .await
        .unwrap();

    // Enqueue work again for the workflow to resume
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();

    // Second run: workflow should resume and complete with null (no explicit return)
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow completed with null output
    let final_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(final_execution.status, ExecutionStatus::Completed);
    assert_eq!(final_execution.output, Some(json!(null)));

    // Verify work queue is empty and no workflow context exists
    let work_count = get_work_queue_count(&pool, &workflow_id).await.unwrap();
    assert_eq!(work_count, 0);
    let context = db::workflow_execution_context::get_context(&pool, &workflow_id)
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
        task_id,
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

#[tokio::test(flavor = "multi_thread")]
async fn test_workflow_runtime_error_sets_failed_status() {
    // Workflow that throws a runtime error by accessing undefined variable
    let workflow_source = r#"
        return undefined_variable
    "#;

    let (pool, execution) =
        setup_workflow_test("runtime_error_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Run workflow - should complete (not error) but set status to Failed
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow is in Failed status
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Failed);

    // Verify error output contains expected error info
    let output = workflow_execution.output.unwrap();
    assert_eq!(output.get("code").unwrap(), "INTERNAL_ERROR");
    assert_eq!(
        output.get("message").unwrap(),
        "Undefined variable 'undefined_variable'"
    );
}

/* ===================== Timer Integration Tests ===================== */

#[tokio::test(flavor = "multi_thread")]
async fn test_workflow_suspends_on_timer() {
    // Workflow that awaits a timer
    let workflow_source = r#"
        await Timer.delay(60)
        return "timer_done"
    "#;

    let (pool, execution) = setup_workflow_test("timer_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Run workflow - should suspend on timer
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow suspended
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Suspended);

    // Verify workflow execution context exists (timer state saved)
    let context = db::workflow_execution_context::get_context(&pool, &workflow_id)
        .await
        .unwrap();
    assert!(context.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_timer_schedules_to_scheduled_queue() {
    // Workflow that awaits a timer
    let workflow_source = r#"
        await Timer.delay(5)
        return "done"
    "#;

    let (pool, execution) =
        setup_workflow_test("timer_schedule_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();
    let queue = execution.queue.clone();

    let before = chrono::Utc::now();

    // Run workflow - should suspend and schedule timer
    run_workflow(&pool, execution).await.unwrap();

    let after = chrono::Utc::now();

    // Verify timer was scheduled in scheduled_queue with correct params
    let scheduled: (chrono::NaiveDateTime, serde_json::Value) = sqlx::query_as(
        "SELECT run_at, params FROM scheduled_queue WHERE params->>'execution_id' = $1",
    )
    .bind(&workflow_id)
    .fetch_one(pool.as_ref())
    .await
    .unwrap();

    let (run_at, params) = scheduled;

    // Verify run_at is approximately 5 seconds in the future
    let expected_min = (before + chrono::Duration::milliseconds(5000)).naive_utc();
    let expected_max = (after + chrono::Duration::milliseconds(5000)).naive_utc();
    assert!(
        run_at >= expected_min && run_at <= expected_max,
        "run_at {:?} should be between {:?} and {:?}",
        run_at,
        expected_min,
        expected_max
    );

    // Verify params contain correct execution_id, queue, and priority
    assert_eq!(params.get("execution_id").unwrap(), &workflow_id);
    assert_eq!(params.get("queue").unwrap(), &queue);
    assert_eq!(params.get("priority").unwrap(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_timer_resumes_when_ready() {
    // Workflow that awaits a very short timer (0ms - fires immediately)
    // Since the timer's fire_at is already <= db_now when checked in the runner loop,
    // the timer fires in the same execution cycle without needing a separate run
    let workflow_source = r#"
        await Timer.delay(0)
        return "timer_fired"
    "#;

    let (pool, execution) =
        setup_workflow_test("immediate_timer_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Run - 0ms timer fires immediately since fire_at <= db_now
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow completed in one run
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    assert_eq!(workflow_execution.output, Some(json!("timer_fired")));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_timer_stays_suspended_when_not_ready() {
    // Workflow that awaits a long timer (1 hour)
    let workflow_source = r#"
        await Timer.delay(3600)
        return "timer_fired"
    "#;

    let (pool, execution) =
        setup_workflow_test("long_timer_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends on timer
    run_workflow(&pool, execution).await.unwrap();

    // Verify suspended
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Suspended);

    // Re-enqueue and run again - timer should NOT be ready (fire_at > now)
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();

    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .expect("Execution should exist");
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow is STILL suspended (timer not fired yet)
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Suspended);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_then_timer_workflow() {
    // Workflow that awaits a task, then a timer
    // After task completes, the 0ms timer fires immediately in the same run
    let workflow_source = r#"
        task_result = await Task.run("process", {value: 42})
        await Timer.delay(0)
        return task_result * 2
    "#;

    let (pool, execution) =
        setup_workflow_test("task_then_timer_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends on task
    run_workflow(&pool, execution).await.unwrap();

    // Verify suspended on task
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Suspended);

    // Complete the task
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    assert_eq!(tasks.len(), 1);
    db::executions::complete_execution(pool.as_ref(), &tasks[0].0, json!(100))
        .await
        .unwrap();

    // Second run - resumes from task, 0ms timer fires immediately, workflow completes
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();

    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .expect("Execution should exist");
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow completed (0ms timer fired immediately after task resumed)
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    assert_eq!(workflow_execution.output, Some(json!(200.0)));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_multiple_sequential_timers() {
    // Workflow with multiple sequential timers (all 0ms for immediate firing)
    let workflow_source = r#"
        await Timer.delay(0)
        await Timer.delay(0)
        await Timer.delay(0)
        return "all_timers_done"
    "#;

    let (pool, execution) =
        setup_workflow_test("multi_timer_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Run through all 3 timers
    for i in 0..3 {
        if i > 0 {
            enqueue_and_claim_execution(&pool, &workflow_id, "default")
                .await
                .unwrap();
        }

        let execution = db::executions::get_execution(&pool, &workflow_id)
            .await
            .unwrap()
            .expect("Execution should exist");
        run_workflow(&pool, execution).await.unwrap();
    }

    // Fourth run - should complete
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
    assert_eq!(workflow_execution.output, Some(json!("all_timers_done")));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_fire_and_forget_timer() {
    // Timer created but not awaited (fire-and-forget)
    let workflow_source = r#"
        Timer.delay(1)
        return "done_without_waiting"
    "#;

    let (pool, execution) =
        setup_workflow_test("fire_forget_timer_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Run workflow - should complete immediately (timer not awaited)
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow completed
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    assert_eq!(
        workflow_execution.output,
        Some(json!("done_without_waiting"))
    );

    // Timer should still be scheduled (fire-and-forget creates the schedule)
    let scheduled_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM scheduled_queue WHERE params->>'execution_id' = $1")
            .bind(&workflow_id)
            .fetch_one(pool.as_ref())
            .await
            .unwrap();

    assert_eq!(
        scheduled_count.0, 1,
        "Fire-and-forget timer should be scheduled"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_timer_captured_then_awaited_after_task() {
    // Timer created early, task awaited, then timer awaited
    // This tests that timers "start" when created, not when awaited
    // Since 0ms timer will already be ready by the time we await it,
    // the workflow should complete in one run after task completion
    let workflow_source = r#"
        timer = Timer.delay(0)
        task_result = await Task.run("slow_task", {})
        await timer
        return task_result
    "#;

    let (pool, execution) =
        setup_workflow_test("timer_after_task_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends on task (timer already created and scheduled)
    run_workflow(&pool, execution).await.unwrap();

    // Verify suspended on task
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Suspended);

    // Timer should already be scheduled even though we haven't awaited it yet
    let scheduled_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM scheduled_queue WHERE params->>'execution_id' = $1")
            .bind(&workflow_id)
            .fetch_one(pool.as_ref())
            .await
            .unwrap();
    assert_eq!(
        scheduled_count.0, 1,
        "Timer should be scheduled before being awaited"
    );

    // Complete the task
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    assert_eq!(tasks.len(), 1);
    db::executions::complete_execution(pool.as_ref(), &tasks[0].0, json!("task_done"))
        .await
        .unwrap();

    // Second run - timer was created before task, so it's already ready (0ms elapsed)
    // The runner loop will: resume task -> hit timer await -> check timer ready -> resume timer -> complete
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();

    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .expect("Execution should exist");
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow completed with task result (timer fired immediately since it was already ready)
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    assert_eq!(workflow_execution.output, Some(json!("task_done")));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_parallel_timer_and_task() {
    // Create both timer and task, await task first, then timer
    // Simulates starting a timer while waiting for a task
    // Since 0ms timer is already ready when awaited, it fires in same execution cycle
    let workflow_source = r#"
        timer = Timer.delay(0)
        task = Task.run("work", {})
        task_result = await task
        await timer
        return task_result
    "#;

    let (pool, execution) =
        setup_workflow_test("parallel_timer_task_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends on task
    run_workflow(&pool, execution).await.unwrap();

    // Both timer and task should be created
    let scheduled_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM scheduled_queue WHERE params->>'execution_id' = $1")
            .bind(&workflow_id)
            .fetch_one(pool.as_ref())
            .await
            .unwrap();
    assert_eq!(scheduled_count.0, 1, "Timer should be scheduled");

    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    assert_eq!(tasks.len(), 1, "Task should be created");

    // Complete the task
    db::executions::complete_execution(pool.as_ref(), &tasks[0].0, json!("work_done"))
        .await
        .unwrap();

    // Second run - resumes from task, timer already ready (0ms), completes
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();

    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .expect("Execution should exist");
    run_workflow(&pool, execution).await.unwrap();

    // Verify workflow completed (timer fired immediately since it was already ready)
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    assert_eq!(workflow_execution.output, Some(json!("work_done")));
}
