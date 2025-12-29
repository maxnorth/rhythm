//! Integration tests for composite awaitable resolution (Promise.all, Promise.any, Promise.race)
//!
//! These tests verify the DB-level resolution logic in awaitable.rs, ensuring that:
//! - Promise.all waits for all tasks and fails fast on error
//! - Promise.any returns first success, fails only if all fail
//! - Promise.race returns first settled (success or error)
//! - Nested composites resolve correctly
//! - Mixed task/timer composites work (e.g., timeout patterns)

use serde_json::json;

use super::super::run_workflow;
use crate::db;
use crate::test_helpers::{enqueue_and_claim_execution, get_child_tasks, setup_workflow_test};
use crate::types::ExecutionStatus;

/* ===================== Promise.all() Integration Tests ===================== */

#[tokio::test(flavor = "multi_thread")]
async fn test_task_all_waits_for_all_tasks() {
    let workflow_source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        let results = await Promise.all([t1, t2])
        return results
    "#;

    let (pool, execution) =
        setup_workflow_test("task_all_workflow", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends on Promise.all
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Suspended);

    // Complete only task1
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    assert_eq!(tasks.len(), 2);

    let task1_id = &tasks[0].0;
    db::executions::complete_execution(pool.as_ref(), task1_id, json!("result1"))
        .await
        .unwrap();

    // Resume - should still be suspended (task2 pending)
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        workflow_execution.status,
        ExecutionStatus::Suspended,
        "Should stay suspended until all tasks complete"
    );

    // Complete task2
    let task2_id = &tasks[1].0;
    db::executions::complete_execution(pool.as_ref(), task2_id, json!("result2"))
        .await
        .unwrap();

    // Resume - should complete now
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    assert_eq!(
        workflow_execution.output,
        Some(json!(["result1", "result2"]))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_all_fails_fast_on_error() {
    let workflow_source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        let results = await Promise.all([t1, t2])
        return results
    "#;

    let (pool, execution) =
        setup_workflow_test("task_all_fail_fast", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends on Promise.all
    run_workflow(&pool, execution).await.unwrap();

    // Fail task1 (task2 still pending)
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    let task1_id = &tasks[0].0;
    db::executions::fail_execution(
        pool.as_ref(),
        task1_id,
        json!({"code": "TASK_FAILED", "message": "Task 1 failed"}),
    )
    .await
    .unwrap();

    // Resume - should complete with error (fail-fast)
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    // The workflow completes because await returns the error value
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    let output = workflow_execution.output.unwrap();
    assert_eq!(output.get("code").unwrap(), "TASK_FAILED");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_all_with_object() {
    let workflow_source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        let results = await Promise.all({ first: t1, second: t2 })
        return results
    "#;

    let (pool, execution) =
        setup_workflow_test("task_all_object", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends
    run_workflow(&pool, execution).await.unwrap();

    // Complete both tasks
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    for (task_id, task_name) in &tasks {
        let result = if task_name == "task1" { "one" } else { "two" };
        db::executions::complete_execution(pool.as_ref(), task_id, json!(result))
            .await
            .unwrap();
    }

    // Resume - should complete with object result
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    let output = workflow_execution.output.unwrap();
    assert_eq!(output.get("first").unwrap(), "one");
    assert_eq!(output.get("second").unwrap(), "two");
}

/* ===================== Promise.any() Integration Tests ===================== */

#[tokio::test(flavor = "multi_thread")]
async fn test_task_any_returns_first_success() {
    let workflow_source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        let result = await Promise.any([t1, t2])
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("task_any_first_success", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends
    run_workflow(&pool, execution).await.unwrap();

    // Complete task2 first (task1 still pending)
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    let task2_id = &tasks[1].0;
    db::executions::complete_execution(pool.as_ref(), task2_id, json!("winner"))
        .await
        .unwrap();

    // Resume - should complete with task2's result
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    // Promise.any returns just the value (not { key, value })
    assert_eq!(workflow_execution.output, Some(json!("winner")));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_any_skips_failures() {
    let workflow_source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        let result = await Promise.any([t1, t2])
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("task_any_skip_failures", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends
    run_workflow(&pool, execution).await.unwrap();

    // Fail task1
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    let task1_id = &tasks[0].0;
    db::executions::fail_execution(pool.as_ref(), task1_id, json!({"error": "failed"}))
        .await
        .unwrap();

    // Resume - should still be suspended (waiting for task2)
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        workflow_execution.status,
        ExecutionStatus::Suspended,
        "Should stay suspended - task1 failed but task2 still pending"
    );

    // Complete task2
    let task2_id = &tasks[1].0;
    db::executions::complete_execution(pool.as_ref(), task2_id, json!("success"))
        .await
        .unwrap();

    // Resume - should complete with task2's result
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    // Promise.any returns just the value (not { key, value })
    assert_eq!(workflow_execution.output, Some(json!("success")));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_any_all_fail_returns_aggregate_error() {
    let workflow_source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        let result = await Promise.any([t1, t2])
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("task_any_all_fail", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends
    run_workflow(&pool, execution).await.unwrap();

    // Fail both tasks
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    for (task_id, _) in &tasks {
        db::executions::fail_execution(pool.as_ref(), task_id, json!({"error": "failed"}))
            .await
            .unwrap();
    }

    // Resume - should complete with AggregateError
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    let output = workflow_execution.output.unwrap();
    assert_eq!(output.get("code").unwrap(), "AggregateError");
}

/* ===================== Promise.race() Integration Tests ===================== */

#[tokio::test(flavor = "multi_thread")]
async fn test_task_race_returns_first_success() {
    let workflow_source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        let result = await Promise.race([t1, t2])
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("task_race_first_success", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends
    run_workflow(&pool, execution).await.unwrap();

    // Complete task1 first
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    let task1_id = &tasks[0].0;
    db::executions::complete_execution(pool.as_ref(), task1_id, json!("first"))
        .await
        .unwrap();

    // Resume - should complete with task1's result
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    // Promise.race returns just the value (not { key, value })
    assert_eq!(workflow_execution.output, Some(json!("first")));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_race_returns_first_error() {
    let workflow_source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        let result = await Promise.race([t1, t2])
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("task_race_first_error", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends
    run_workflow(&pool, execution).await.unwrap();

    // Fail task1 first (task2 still pending)
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    let task1_id = &tasks[0].0;
    db::executions::fail_execution(
        pool.as_ref(),
        task1_id,
        json!({"code": "RACE_LOSER", "message": "Failed first"}),
    )
    .await
    .unwrap();

    // Resume - should complete with error (race propagates first error)
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    // Race propagates the error, so workflow returns the error value
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    let output = workflow_execution.output.unwrap();
    assert_eq!(output.get("code").unwrap(), "RACE_LOSER");
}

/* ===================== Mixed Composite Tests ===================== */

#[tokio::test(flavor = "multi_thread")]
async fn test_task_race_with_timer_timeout_pattern() {
    // Race between a task and a 0ms timer (immediate timeout)
    let workflow_source = r#"
        let task = Task.run("slow_task", {})
        let timeout = Timer.delay(0)
        let result = await Promise.race([task, timeout])
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("race_timeout_pattern", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Run - timer fires immediately (0ms), should win the race
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    // Timer won, Promise.race returns just the value (null for timers)
    assert_eq!(workflow_execution.output, Some(json!(null)));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_all_with_timer() {
    // Wait for both a task and a timer
    let workflow_source = r#"
        let task = Task.run("some_task", {})
        let timer = Timer.delay(0)
        let results = await Promise.all([task, timer])
        return results
    "#;

    let (pool, execution) = setup_workflow_test("all_with_timer", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - timer fires but task pending, so still suspended
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Suspended);

    // Complete the task
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    assert_eq!(tasks.len(), 1);
    db::executions::complete_execution(pool.as_ref(), &tasks[0].0, json!("task_done"))
        .await
        .unwrap();

    // Resume - both ready now, should complete
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    assert_eq!(workflow_execution.output, Some(json!(["task_done", null])));
}

/* ===================== Nested Composite Tests ===================== */

#[tokio::test(flavor = "multi_thread")]
async fn test_nested_all_in_race() {
    // Race between Promise.all([t1, t2]) and a timer
    let workflow_source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        let all_tasks = Promise.all([t1, t2])
        let timeout = Timer.delay(0)
        let result = await Promise.race([all_tasks, timeout])
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("nested_all_in_race", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Run - timer fires immediately, wins the race before tasks complete
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    // Timer won, Promise.race returns just the value (null)
    assert_eq!(workflow_execution.output, Some(json!(null)));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_nested_race_in_all() {
    // Wait for all of: Promise.race([t1, timer]), t2
    let workflow_source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        let timer = Timer.delay(0)
        let race_result = Promise.race([t1, timer])
        let results = await Promise.all([race_result, t2])
        return results
    "#;

    let (pool, execution) =
        setup_workflow_test("nested_race_in_all", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - race completes (timer wins), but t2 still pending
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Suspended);

    // Complete t2
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    // Find task2 (the one not in the race)
    let task2 = tasks.iter().find(|(_, name)| name == "task2").unwrap();
    db::executions::complete_execution(pool.as_ref(), &task2.0, json!("task2_done"))
        .await
        .unwrap();

    // Resume - both ready now
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    // First element is race result (null - timer won), second is "task2_done"
    assert_eq!(workflow_execution.output, Some(json!([null, "task2_done"])));
}

/* ===================== Promise.any_kv() Integration Tests ===================== */

#[tokio::test(flavor = "multi_thread")]
async fn test_task_any_kv_returns_key_and_value() {
    let workflow_source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        let result = await Promise.any_kv([t1, t2])
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("task_any_kv_first_success", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends
    run_workflow(&pool, execution).await.unwrap();

    // Complete task2 first (task1 still pending)
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    let task2_id = &tasks[1].0;
    db::executions::complete_execution(pool.as_ref(), task2_id, json!("winner"))
        .await
        .unwrap();

    // Resume - should complete with { key, value }
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    // Promise.any_kv returns { key, value }
    let output = workflow_execution.output.unwrap();
    assert_eq!(output.get("key").unwrap(), &json!(1.0));
    assert_eq!(output.get("value").unwrap(), "winner");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_any_kv_with_object() {
    let workflow_source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        let result = await Promise.any_kv({ first: t1, second: t2 })
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("task_any_kv_object", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends
    run_workflow(&pool, execution).await.unwrap();

    // Complete task1
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    let task1 = tasks.iter().find(|(_, name)| name == "task1").unwrap();
    db::executions::complete_execution(pool.as_ref(), &task1.0, json!("first_wins"))
        .await
        .unwrap();

    // Resume
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    // For object input, key is a string
    let output = workflow_execution.output.unwrap();
    assert_eq!(output.get("key").unwrap(), "first");
    assert_eq!(output.get("value").unwrap(), "first_wins");
}

/* ===================== Promise.race_kv() Integration Tests ===================== */

#[tokio::test(flavor = "multi_thread")]
async fn test_task_race_kv_returns_key_and_value() {
    let workflow_source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        let result = await Promise.race_kv([t1, t2])
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("task_race_kv_first_success", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends
    run_workflow(&pool, execution).await.unwrap();

    // Complete task1 first
    let tasks = get_child_tasks(&pool, &workflow_id).await.unwrap();
    let task1_id = &tasks[0].0;
    db::executions::complete_execution(pool.as_ref(), task1_id, json!("first"))
        .await
        .unwrap();

    // Resume - should complete with { key, value }
    enqueue_and_claim_execution(&pool, &workflow_id, "default")
        .await
        .unwrap();
    let execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    // Promise.race_kv returns { key, value }
    let output = workflow_execution.output.unwrap();
    assert_eq!(output.get("key").unwrap(), &json!(0.0));
    assert_eq!(output.get("value").unwrap(), "first");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_race_kv_with_timer_timeout_pattern() {
    // Race between a task and a 0ms timer (immediate timeout)
    let workflow_source = r#"
        let task = Task.run("slow_task", {})
        let timeout = Timer.delay(0)
        let result = await Promise.race_kv([task, timeout])
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("race_kv_timeout_pattern", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Run - timer fires immediately (0ms), should win the race
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    let output = workflow_execution.output.unwrap();
    // Timer won (index 1), value is null
    assert_eq!(output.get("key").unwrap(), &json!(1.0));
    assert_eq!(output.get("value").unwrap(), &json!(null));
}
