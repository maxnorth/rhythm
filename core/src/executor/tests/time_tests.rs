//! Tests for Time.delay() and timer functionality

use super::helpers::parse_workflow_and_build_vm;
use crate::executor::{errors, run_until_done, Awaitable, Control, Val, VM};
use chrono::{Duration, Utc};
use std::collections::HashMap;

/* ===================== Time.delay() Basic Tests ===================== */

#[test]
fn test_time_delay_basic() {
    // Time.delay(1000) returns a Promise(Timer) with fire_at ~1 second in the future
    let source = r#"
        return Time.delay(1000)
    "#;

    let before = Utc::now();
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    let after = Utc::now();

    // Should return a Promise(Timer) value
    match &vm.control {
        Control::Return(Val::Promise(Awaitable::Timer { fire_at })) => {
            // fire_at should be approximately 1 second after now
            let expected_min = before + Duration::milliseconds(1000);
            let expected_max = after + Duration::milliseconds(1000);
            assert!(
                *fire_at >= expected_min && *fire_at <= expected_max,
                "fire_at {:?} should be between {:?} and {:?}",
                fire_at,
                expected_min,
                expected_max
            );
        }
        _ => panic!(
            "Expected Control::Return(Val::Promise(Awaitable::Timer {{ .. }})), got {:?}",
            vm.control
        ),
    }

    // Check outbox has one timer
    assert_eq!(vm.outbox.timers.len(), 1);
}

#[test]
fn test_time_delay_zero() {
    // Time.delay(0) should work and set fire_at to approximately now
    let source = r#"
        return Time.delay(0)
    "#;

    let before = Utc::now();
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    let after = Utc::now();

    match &vm.control {
        Control::Return(Val::Promise(Awaitable::Timer { fire_at })) => {
            // fire_at should be between before and after (immediate)
            assert!(
                *fire_at >= before && *fire_at <= after,
                "fire_at {:?} should be between {:?} and {:?}",
                fire_at,
                before,
                after
            );
        }
        _ => panic!("Expected Timer, got {:?}", vm.control),
    }

    assert_eq!(vm.outbox.timers.len(), 1);
}

#[test]
fn test_time_delay_large_value() {
    // Time.delay(3600000) - 1 hour
    let source = r#"
        return Time.delay(3600000)
    "#;

    let before = Utc::now();
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    match &vm.control {
        Control::Return(Val::Promise(Awaitable::Timer { fire_at })) => {
            // fire_at should be approximately 1 hour in the future
            let expected = before + Duration::hours(1);
            let diff = (*fire_at - expected).num_seconds().abs();
            assert!(
                diff < 2,
                "fire_at should be ~1 hour from now, diff was {} seconds",
                diff
            );
        }
        _ => panic!("Expected Timer, got {:?}", vm.control),
    }
}

/* ===================== Outbox Side Effects Tests ===================== */

#[test]
fn test_time_delay_records_timer_in_outbox() {
    let source = r#"
        return Time.delay(5000)
    "#;

    let before = Utc::now();
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Check outbox has the timer
    assert_eq!(vm.outbox.timers.len(), 1);
    let timer = &vm.outbox.timers[0];

    // Timer fire_at should match what's in the return value
    if let Control::Return(Val::Promise(Awaitable::Timer { fire_at })) = &vm.control {
        assert_eq!(timer.fire_at, *fire_at);
    }

    // Timer should be ~5 seconds in the future
    let expected = before + Duration::milliseconds(5000);
    let diff = (timer.fire_at - expected).num_milliseconds().abs();
    assert!(
        diff < 100,
        "Timer fire_at should be ~5s in future, diff was {}ms",
        diff
    );
}

#[test]
fn test_time_delay_multiple_timers() {
    // Multiple Time.delay() calls add multiple entries to outbox
    let source = r#"
        Time.delay(1000)
        Time.delay(2000)
        return Time.delay(3000)
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should have 3 timers in outbox
    assert_eq!(vm.outbox.timers.len(), 3);

    // Each should have different fire_at times (increasing)
    assert!(vm.outbox.timers[0].fire_at < vm.outbox.timers[1].fire_at);
    assert!(vm.outbox.timers[1].fire_at < vm.outbox.timers[2].fire_at);
}

/* ===================== Await/Suspend Tests ===================== */

#[test]
fn test_await_time_delay_suspends() {
    // await Time.delay(1000) should suspend the VM
    let source = r#"
        return await Time.delay(1000)
    "#;

    let before = Utc::now();
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should be suspended on a timer
    match &vm.control {
        Control::Suspend(Awaitable::Timer { fire_at }) => {
            let expected = before + Duration::milliseconds(1000);
            let diff = (*fire_at - expected).num_milliseconds().abs();
            assert!(
                diff < 100,
                "fire_at should be ~1s in future, diff was {}ms",
                diff
            );
        }
        _ => panic!(
            "Expected Control::Suspend(Awaitable::Timer {{ .. }}), got {:?}",
            vm.control
        ),
    }

    // Frames should be preserved (not popped due to suspension)
    assert_eq!(vm.frames.len(), 2); // Block + Return frames
}

#[test]
fn test_await_time_delay_resume() {
    // After resuming a suspended timer, execution continues
    let source = r#"
        let result = await Time.delay(1000)
        return result
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should be suspended
    assert!(matches!(
        vm.control,
        Control::Suspend(Awaitable::Timer { .. })
    ));

    // Serialize and deserialize (like real workflow resumption)
    let serialized = serde_json::to_string(&vm).unwrap();
    let mut vm2: VM = serde_json::from_str(&serialized).unwrap();

    // Resume with null (timers resume with null)
    assert!(vm2.resume(Val::Null));
    run_until_done(&mut vm2);

    // Should return null
    assert_eq!(vm2.control, Control::Return(Val::Null));
}

#[test]
fn test_await_time_delay_in_sequence() {
    // Multiple awaited sleeps in sequence
    let source = r#"
        await Time.delay(100)
        await Time.delay(200)
        return "done"
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should suspend on first timer
    assert!(matches!(
        vm.control,
        Control::Suspend(Awaitable::Timer { .. })
    ));

    // Resume first timer
    vm.resume(Val::Null);
    run_until_done(&mut vm);

    // Should suspend on second timer
    assert!(matches!(
        vm.control,
        Control::Suspend(Awaitable::Timer { .. })
    ));

    // Resume second timer
    vm.resume(Val::Null);
    run_until_done(&mut vm);

    // Should return "done"
    assert_eq!(vm.control, Control::Return(Val::Str("done".to_string())));
}

#[test]
fn test_timer_serialization() {
    // Test that suspended timer state can be serialized/deserialized
    let source = r#"
        return await Time.delay(60000)
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Capture the fire_at before serialization
    let original_fire_at = match &vm.control {
        Control::Suspend(Awaitable::Timer { fire_at }) => *fire_at,
        _ => panic!("Expected suspended timer"),
    };

    // Serialize
    let serialized = serde_json::to_string(&vm).unwrap();

    // Deserialize
    let vm2: VM = serde_json::from_str(&serialized).unwrap();

    // Check fire_at is preserved
    match &vm2.control {
        Control::Suspend(Awaitable::Timer { fire_at }) => {
            assert_eq!(*fire_at, original_fire_at);
        }
        _ => panic!("Expected suspended timer after deserialization"),
    }
}

/* ===================== Error Handling Tests ===================== */

#[test]
fn test_time_delay_wrong_arg_count_zero() {
    // Time.delay() with no arguments
    let source = r#"
        return Time.delay()
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_COUNT);
    assert!(err.message.contains("Expected 1 argument"));
}

#[test]
fn test_time_delay_wrong_arg_count_two() {
    // Time.delay(1000, 2000) - too many arguments
    let source = r#"
        return Time.delay(1000, 2000)
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_COUNT);
    assert!(err.message.contains("Expected 1 argument"));
}

#[test]
fn test_time_delay_wrong_arg_type_string() {
    // Time.delay("1000") - string instead of number
    let source = r#"
        return Time.delay("1000")
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_TYPE);
    assert!(err.message.contains("number"));
}

#[test]
fn test_time_delay_wrong_arg_type_null() {
    // Time.delay(null) - null instead of number
    let source = r#"
        return Time.delay(null)
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_TYPE);
}

#[test]
fn test_time_delay_negative_duration() {
    // Time.delay(-1000) - negative duration
    let source = r#"
        return Time.delay(-1000)
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_TYPE);
    assert!(err.message.contains("non-negative"));
}

/* ===================== Combined Task and Timer Tests ===================== */

#[test]
fn test_task_and_timer_in_same_workflow() {
    // Test that tasks and timers can coexist in the same workflow
    let source = r#"
        Task.run("my_task", {})
        Time.delay(1000)
        return "done"
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Outbox should have both
    assert_eq!(vm.outbox.tasks.len(), 1);
    assert_eq!(vm.outbox.timers.len(), 1);

    assert_eq!(vm.outbox.tasks[0].task_name, "my_task");
}

#[test]
fn test_await_task_then_timer() {
    // await task, then await timer
    let source = r#"
        let task_result = await Task.run("process", {})
        await Time.delay(1000)
        return task_result
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should suspend on task first
    match &vm.control {
        Control::Suspend(Awaitable::Task(task_id)) => {
            assert_eq!(task_id.len(), 36); // UUID
        }
        _ => panic!("Expected suspended on task, got {:?}", vm.control),
    }

    // Resume task with result
    vm.resume(Val::Str("task_done".to_string()));
    run_until_done(&mut vm);

    // Should now suspend on timer
    assert!(matches!(
        vm.control,
        Control::Suspend(Awaitable::Timer { .. })
    ));

    // Resume timer
    vm.resume(Val::Null);
    run_until_done(&mut vm);

    // Should return task result
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("task_done".to_string()))
    );
}
