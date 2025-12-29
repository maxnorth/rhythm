//! Integration tests for signal handling in workflows
//!
//! These tests verify the end-to-end signal flow:
//! - Workflow suspends on Signal.next()
//! - External signal arrives via send_signal
//! - Workflow resumes with signal payload
//! - Race conditions are handled correctly

use serde_json::json;

use super::super::run_workflow;
use crate::db;
use crate::test_helpers::{enqueue_and_claim_execution, setup_workflow_test};
use crate::types::ExecutionStatus;

/* ===================== Basic Signal Flow Tests ===================== */

#[tokio::test(flavor = "multi_thread")]
async fn test_signal_workflow_suspends_waiting_for_signal() {
    let workflow_source = r#"
        let result = await Signal.next("approval")
        return result
    "#;

    let (pool, execution) = setup_workflow_test("signal_suspend", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - should suspend waiting for signal
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Suspended);

    // Verify a 'requested' signal was created
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM signals WHERE workflow_id = $1 AND status = 'requested'",
    )
    .bind(&workflow_id)
    .fetch_one(pool.as_ref())
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_signal_workflow_resumes_with_payload() {
    let workflow_source = r#"
        let result = await Signal.next("approval")
        return { received: result }
    "#;

    let (pool, execution) = setup_workflow_test("signal_resume", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends waiting for signal
    run_workflow(&pool, execution).await.unwrap();

    // Send a signal
    db::signals::send_signal(
        pool.as_ref(),
        &workflow_id,
        "approval",
        &json!({"approved": true, "reviewer": "alice"}),
    )
    .await
    .unwrap();

    // Resume workflow - resolve_signal_claims will match the signal
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
    let received = output.get("received").unwrap();
    assert_eq!(received.get("approved").unwrap(), true);
    assert_eq!(received.get("reviewer").unwrap(), "alice");
}

/* ===================== Race Condition Tests ===================== */

#[tokio::test(flavor = "multi_thread")]
async fn test_signal_sent_before_workflow_reaches_next() {
    // Signal arrives BEFORE workflow is even started
    // This tests the race condition where send_signal runs before Signal.next()

    let workflow_source = r#"
        let result = await Signal.next("early_signal")
        return result
    "#;

    let (pool, execution) = setup_workflow_test("signal_early", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Send the signal BEFORE running the workflow
    db::signals::send_signal(
        pool.as_ref(),
        &workflow_id,
        "early_signal",
        &json!({"early": true}),
    )
    .await
    .unwrap();

    // Now run the workflow - it should find the signal and complete immediately
    // (after first run creates the request and second run resolves it)
    run_workflow(&pool, execution).await.unwrap();

    // First run suspends (creates request, but signal was sent before)
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();

    // Need to resume to trigger resolve_signal_claims
    if workflow_execution.status == ExecutionStatus::Suspended {
        enqueue_and_claim_execution(&pool, &workflow_id, "default")
            .await
            .unwrap();
        let execution = db::executions::get_execution(&pool, &workflow_id)
            .await
            .unwrap()
            .unwrap();
        run_workflow(&pool, execution).await.unwrap();
    }

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);
    assert_eq!(workflow_execution.output, Some(json!({"early": true})));
}

/* ===================== Multiple Signals Tests ===================== */

#[tokio::test(flavor = "multi_thread")]
async fn test_multiple_signals_same_channel_fifo() {
    // Send multiple signals, then wait for them - should get FIFO order
    let workflow_source = r#"
        let first = await Signal.next("queue")
        let second = await Signal.next("queue")
        return { first: first, second: second }
    "#;

    let (pool, execution) = setup_workflow_test("signal_fifo", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Send two signals before workflow runs
    db::signals::send_signal(pool.as_ref(), &workflow_id, "queue", &json!({"order": 1}))
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    db::signals::send_signal(pool.as_ref(), &workflow_id, "queue", &json!({"order": 2}))
        .await
        .unwrap();

    // Run workflow - with both signals pre-sent, should complete in one run
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();

    // If still suspended, try one more resume
    let workflow_execution = if workflow_execution.status == ExecutionStatus::Suspended {
        enqueue_and_claim_execution(&pool, &workflow_id, "default")
            .await
            .unwrap();
        let execution = db::executions::get_execution(&pool, &workflow_id)
            .await
            .unwrap()
            .unwrap();
        run_workflow(&pool, execution).await.unwrap();
        db::executions::get_execution(&pool, &workflow_id)
            .await
            .unwrap()
            .unwrap()
    } else {
        workflow_execution
    };

    assert_eq!(
        workflow_execution.status,
        ExecutionStatus::Completed,
        "Workflow should complete after processing both signals"
    );

    let output = workflow_execution.output.unwrap();
    // The workflow runtime converts integers to floats
    assert_eq!(output["first"]["order"], 1.0);
    assert_eq!(output["second"]["order"], 2.0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_present_signals_matched_in_fifo_order() {
    // Signals are sent BEFORE the workflow requests them
    // When workflow calls Signal.next multiple times, it should match in FIFO order
    let workflow_source = r#"
        let first = await Signal.next("queue")
        let second = await Signal.next("queue")
        let third = await Signal.next("queue")
        return { first: first, second: second, third: third }
    "#;

    let (pool, execution) = setup_workflow_test("presend_fifo", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Send 3 signals BEFORE workflow runs (with delays to ensure ordering)
    db::signals::send_signal(pool.as_ref(), &workflow_id, "queue", &json!({"order": 1}))
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    db::signals::send_signal(pool.as_ref(), &workflow_id, "queue", &json!({"order": 2}))
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    db::signals::send_signal(pool.as_ref(), &workflow_id, "queue", &json!({"order": 3}))
        .await
        .unwrap();

    // Run workflow - all signals exist, so match_outbox_signals_to_unclaimed should match them
    run_workflow(&pool, execution).await.unwrap();

    // First run may suspend because Signal.next creates a request first
    // Resume until complete
    let mut workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();

    let mut iterations = 0;
    while workflow_execution.status == ExecutionStatus::Suspended && iterations < 5 {
        enqueue_and_claim_execution(&pool, &workflow_id, "default")
            .await
            .unwrap();
        let execution = db::executions::get_execution(&pool, &workflow_id)
            .await
            .unwrap()
            .unwrap();
        run_workflow(&pool, execution).await.unwrap();

        workflow_execution = db::executions::get_execution(&pool, &workflow_id)
            .await
            .unwrap()
            .unwrap();
        iterations += 1;
    }

    assert_eq!(
        workflow_execution.status,
        ExecutionStatus::Completed,
        "Workflow should complete after matching all pre-sent signals"
    );

    // Verify FIFO order: first Signal.next got oldest, etc.
    let output = workflow_execution.output.unwrap();
    assert_eq!(
        output["first"]["order"], 1.0,
        "First await should get oldest signal"
    );
    assert_eq!(
        output["second"]["order"], 2.0,
        "Second await should get second oldest"
    );
    assert_eq!(
        output["third"]["order"], 3.0,
        "Third await should get newest signal"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_signals_different_channels_no_cross_match() {
    // Signals on different channels should not cross-match
    let workflow_source = r#"
        let approval = await Signal.next("approval")
        return approval
    "#;

    let (pool, execution) =
        setup_workflow_test("signal_channels", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Send signal on WRONG channel
    db::signals::send_signal(
        pool.as_ref(),
        &workflow_id,
        "rejection",
        &json!({"wrong": true}),
    )
    .await
    .unwrap();

    // Run workflow - should suspend (no matching signal)
    run_workflow(&pool, execution).await.unwrap();

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        workflow_execution.status,
        ExecutionStatus::Suspended,
        "Should stay suspended - signal is on wrong channel"
    );

    // Now send signal on correct channel
    db::signals::send_signal(
        pool.as_ref(),
        &workflow_id,
        "approval",
        &json!({"correct": true}),
    )
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
    assert_eq!(workflow_execution.output, Some(json!({"correct": true})));
}

/* ===================== Signal with Other Awaitables Tests ===================== */

#[tokio::test(flavor = "multi_thread")]
async fn test_signal_after_task() {
    // Task completes, then wait for signal
    let workflow_source = r#"
        let task_result = await Task.run("some_task", {})
        let signal = await Signal.next("after_task")
        return { task: task_result, signal: signal }
    "#;

    let (pool, execution) =
        setup_workflow_test("signal_after_task", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // First run - suspends on task
    run_workflow(&pool, execution).await.unwrap();

    // Complete the task
    let tasks: Vec<(String, String)> =
        sqlx::query_as("SELECT id, target_name FROM executions WHERE parent_workflow_id = $1")
            .bind(&workflow_id)
            .fetch_all(pool.as_ref())
            .await
            .unwrap();
    assert_eq!(tasks.len(), 1);

    db::executions::complete_execution(pool.as_ref(), &tasks[0].0, json!("task_done"))
        .await
        .unwrap();

    // Resume - now suspends on signal
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
    assert_eq!(workflow_execution.status, ExecutionStatus::Suspended);

    // Send signal
    db::signals::send_signal(
        pool.as_ref(),
        &workflow_id,
        "after_task",
        &json!({"signal": "received"}),
    )
    .await
    .unwrap();

    // Resume - should complete
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
    assert_eq!(output["task"], "task_done");
    assert_eq!(output["signal"]["signal"], "received");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_signal_in_race_with_timer() {
    // Race between signal and timer - timer wins if signal not sent
    let workflow_source = r#"
        let signal = Signal.next("maybe")
        let timeout = Timer.delay(0)
        let result = await Promise.race([signal, timeout])
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("signal_race_timer", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Don't send any signal - timer should win
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
async fn test_signal_in_race_signal_wins() {
    // Race between signal and timer - signal wins if sent before workflow runs
    let workflow_source = r#"
        let signal = Signal.next("fast")
        let timeout = Timer.delay(60000)
        let result = await Promise.race([signal, timeout])
        return result
    "#;

    let (pool, execution) =
        setup_workflow_test("signal_race_wins", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Send signal before running
    db::signals::send_signal(
        pool.as_ref(),
        &workflow_id,
        "fast",
        &json!({"winner": "signal"}),
    )
    .await
    .unwrap();

    // Run workflow
    run_workflow(&pool, execution).await.unwrap();

    // First run suspends, need to resume to resolve
    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();

    if workflow_execution.status == ExecutionStatus::Suspended {
        enqueue_and_claim_execution(&pool, &workflow_id, "default")
            .await
            .unwrap();
        let execution = db::executions::get_execution(&pool, &workflow_id)
            .await
            .unwrap()
            .unwrap();
        run_workflow(&pool, execution).await.unwrap();
    }

    let workflow_execution = db::executions::get_execution(&pool, &workflow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_execution.status, ExecutionStatus::Completed);

    // Signal won, Promise.race returns just the value
    assert_eq!(workflow_execution.output, Some(json!({"winner": "signal"})));
}

/* ===================== Signal Payload Types Tests ===================== */

#[tokio::test(flavor = "multi_thread")]
async fn test_signal_with_complex_payload() {
    let workflow_source = r#"
        let data = await Signal.next("complex")
        return data
    "#;

    let (pool, execution) =
        setup_workflow_test("signal_complex_payload", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    // Run to suspend
    run_workflow(&pool, execution).await.unwrap();

    // Send complex payload
    let complex_payload = json!({
        "string": "hello",
        "number": 42,
        "float": 2.5,
        "bool": true,
        "null": null,
        "array": [1, 2, 3],
        "nested": {
            "a": {"b": {"c": "deep"}}
        }
    });

    db::signals::send_signal(pool.as_ref(), &workflow_id, "complex", &complex_payload)
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

    // The workflow runtime converts integers to floats, so create expected output with floats
    let expected_output = json!({
        "string": "hello",
        "number": 42.0,
        "float": 2.5,
        "bool": true,
        "null": null,
        "array": [1.0, 2.0, 3.0],
        "nested": {
            "a": {"b": {"c": "deep"}}
        }
    });
    assert_eq!(workflow_execution.output, Some(expected_output));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_signal_with_null_payload() {
    let workflow_source = r#"
        let data = await Signal.next("null_signal")
        return { received: data }
    "#;

    let (pool, execution) =
        setup_workflow_test("signal_null_payload", workflow_source, json!({})).await;
    let workflow_id = execution.id.clone();

    run_workflow(&pool, execution).await.unwrap();

    db::signals::send_signal(pool.as_ref(), &workflow_id, "null_signal", &json!(null))
        .await
        .unwrap();

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
    assert_eq!(workflow_execution.output, Some(json!({"received": null})));
}
