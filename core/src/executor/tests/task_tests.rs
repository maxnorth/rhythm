//! Tests for Task.run() and outbox functionality

use super::helpers::parse_workflow_and_build_vm;
use crate::executor::{errors, run_until_done, Awaitable, Control, Val};
use std::collections::HashMap;

/* ===================== Task.run() Tests ===================== */

#[test]
fn test_task_run_basic() {
    // Task.run("my_task", {input: 42})
    let source = r#"
            return Task.run("my_task", Inputs)
        "#;

    let mut env = HashMap::new();
    env.insert("input".to_string(), Val::Num(42.0));

    let mut vm = parse_workflow_and_build_vm(source, env);
    run_until_done(&mut vm);

    let mut inputs_obj = HashMap::new();
    inputs_obj.insert("input".to_string(), Val::Num(42.0));

    // Should return a Promise(Task) value with a UUID
    match &vm.control {
        Control::Return(Val::Promise(Awaitable::Task(task_id))) => {
            // Task ID should be a valid UUID format (36 characters with dashes)
            assert_eq!(task_id.len(), 36);
            assert!(task_id.contains('-'));
        }
        _ => panic!(
            "Expected Control::Return(Val::Promise(Awaitable::Task(_))), got {:?}",
            vm.control
        ),
    }

    // Check outbox has one task creation
    assert_eq!(vm.outbox.tasks.len(), 1);
    let task_creation = &vm.outbox.tasks[0];
    assert_eq!(task_creation.task_name, "my_task");
    assert_eq!(task_creation.inputs, inputs_obj);

    // Task ID in outbox should match task ID in return value
    if let Control::Return(Val::Promise(Awaitable::Task(task_id))) = &vm.control {
        assert_eq!(task_creation.task_id, *task_id);
    }
}

#[test]
fn test_task_run_empty_inputs() {
    // Task.run("simple_task", {})
    let source = r#"
            return Task.run("simple_task", Inputs)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise(Task) value
    assert!(matches!(
        vm.control,
        Control::Return(Val::Promise(Awaitable::Task(_)))
    ));

    // Check outbox
    assert_eq!(vm.outbox.tasks.len(), 1);
    assert_eq!(vm.outbox.tasks[0].task_name, "simple_task");
    assert_eq!(vm.outbox.tasks[0].inputs, HashMap::new());
}

#[test]
fn test_task_run_multiple_calls() {
    // Create two tasks in sequence
    let source = r#"
            Task.run("first_task", Inputs)
            return Task.run("second_task", Inputs)
        "#;

    let mut env = HashMap::new();
    env.insert("value".to_string(), Val::Num(123.0));

    let mut vm = parse_workflow_and_build_vm(source, env);
    run_until_done(&mut vm);

    let mut inputs_obj = HashMap::new();
    inputs_obj.insert("value".to_string(), Val::Num(123.0));

    // Check outbox has two task creations
    assert_eq!(vm.outbox.tasks.len(), 2);
    assert_eq!(vm.outbox.tasks[0].task_name, "first_task");
    assert_eq!(vm.outbox.tasks[1].task_name, "second_task");

    // Task IDs should be different
    assert_ne!(vm.outbox.tasks[0].task_id, vm.outbox.tasks[1].task_id);
}

#[test]
fn test_fire_and_forget_then_await() {
    // Fire-and-forget one task, then await another
    // This tests that:
    // 1. Fire-and-forget tasks are recorded in the outbox
    // 2. Awaited tasks also get recorded in the outbox
    // 3. VM suspends on the await, preserving state
    let source = r#"
            Task.run("fire_and_forget_task", inputs1)
            return await Task.run("awaited_task", inputs2)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());

    // Manually add inputs1 and inputs2 to env (not parameters, just env variables)
    let mut inputs1 = HashMap::new();
    inputs1.insert("background".to_string(), Val::Bool(true));
    vm.env
        .insert("inputs1".to_string(), Val::Obj(inputs1.clone()));

    let mut inputs2 = HashMap::new();
    inputs2.insert("foreground".to_string(), Val::Bool(true));
    vm.env
        .insert("inputs2".to_string(), Val::Obj(inputs2.clone()));

    run_until_done(&mut vm);

    // VM should be suspended on the awaited task
    match &vm.control {
        Control::Suspend(Awaitable::Task(task_id)) => {
            // Should be suspended on the second task (the awaited one)
            assert_eq!(task_id.len(), 36);
        }
        _ => panic!(
            "Expected Control::Suspend(Awaitable::Task(_)), got {:?}",
            vm.control
        ),
    }

    // Outbox should contain BOTH tasks
    assert_eq!(vm.outbox.tasks.len(), 2);

    // First task (fire-and-forget)
    assert_eq!(vm.outbox.tasks[0].task_name, "fire_and_forget_task");
    assert_eq!(vm.outbox.tasks[0].inputs, inputs1);

    // Second task (awaited)
    assert_eq!(vm.outbox.tasks[1].task_name, "awaited_task");
    assert_eq!(vm.outbox.tasks[1].inputs, inputs2);

    // The suspended task ID should match the second task in the outbox
    if let Control::Suspend(Awaitable::Task(suspended_id)) = &vm.control {
        assert_eq!(vm.outbox.tasks[1].task_id, *suspended_id);
    }

    // Task IDs should be different
    assert_ne!(vm.outbox.tasks[0].task_id, vm.outbox.tasks[1].task_id);

    // Frames should be preserved (not popped due to suspension)
    assert_eq!(vm.frames.len(), 2); // Block + Return frames
}

/* ===================== Task.run() Error Tests ===================== */

#[test]
fn test_task_run_wrong_arg_count_one_arg() {
    // Task.run("my_task") - missing inputs argument
    let source = r#"
            return Task.run("my_task")
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw WRONG_ARG_COUNT error
    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_COUNT);
    assert!(err.message.contains("Expected 2 arguments"));
}

#[test]
fn test_task_run_wrong_arg_count_three_args() {
    // Task.run("my_task", {}, extra) - too many arguments
    let source = r#"
            obj = {}
            return Task.run("my_task", obj, 42)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw WRONG_ARG_COUNT error
    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_COUNT);
    assert!(err.message.contains("Expected 2 arguments, got 3"));
}

#[test]
fn test_task_run_first_arg_not_string() {
    // Task.run(42, {}) - task_name must be string
    let source = r#"
            obj = {}
            return Task.run(42, obj)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw WRONG_ARG_TYPE error
    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_TYPE);
    assert!(err.message.contains("task_name"));
    assert!(err.message.contains("string"));
}

#[test]
fn test_task_run_second_arg_not_object() {
    // Task.run("my_task", 42) - inputs must be object
    let source = r#"
            return Task.run("my_task", 42)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw WRONG_ARG_TYPE error
    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_TYPE);
    assert!(err.message.contains("inputs"));
    assert!(err.message.contains("object"));
}
