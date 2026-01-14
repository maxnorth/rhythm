//! Tests for Workflow.run() and sub-workflow functionality

use super::helpers::parse_workflow_and_build_vm;
use crate::executor::{errors, run_until_done, Awaitable, Control, Val};
use crate::types::ExecutionType;
use std::collections::HashMap;

/* ===================== Workflow.run() Tests ===================== */

#[test]
fn test_workflow_run_basic() {
    // Workflow.run("my_workflow", {input: 42})
    let source = r#"
            return Workflow.run("my_workflow", Inputs)
        "#;

    let mut env = HashMap::new();
    env.insert("input".to_string(), Val::Num(42.0));

    let mut vm = parse_workflow_and_build_vm(source, env);
    run_until_done(&mut vm);

    let mut inputs_obj = HashMap::new();
    inputs_obj.insert("input".to_string(), Val::Num(42.0));

    // Should return a Promise value with a UUID
    match &vm.control {
        Control::Return(Val::Promise(Awaitable::Execution(exec_id))) => {
            // Execution ID should be a valid UUID format (36 characters with dashes)
            assert_eq!(exec_id.len(), 36);
            assert!(exec_id.contains('-'));
        }
        _ => panic!(
            "Expected Control::Return(Val::Promise(Awaitable::Execution(_))), got {:?}",
            vm.control
        ),
    }

    // Check outbox has one execution creation
    assert_eq!(vm.outbox.executions.len(), 1);
    let exec = &vm.outbox.executions[0];
    assert_eq!(exec.target_name, "my_workflow");
    assert_eq!(exec.inputs, inputs_obj);
    assert_eq!(exec.target_type, ExecutionType::Workflow);

    // Execution ID in outbox should match ID in return value
    if let Control::Return(Val::Promise(Awaitable::Execution(exec_id))) = &vm.control {
        assert_eq!(exec.id, *exec_id);
    }
}

#[test]
fn test_workflow_run_empty_inputs() {
    // Workflow.run("simple_workflow", {})
    let source = r#"
            return Workflow.run("simple_workflow", Inputs)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise value
    assert!(matches!(
        vm.control,
        Control::Return(Val::Promise(Awaitable::Execution(_)))
    ));

    // Check outbox
    assert_eq!(vm.outbox.executions.len(), 1);
    assert_eq!(vm.outbox.executions[0].target_name, "simple_workflow");
    assert_eq!(vm.outbox.executions[0].inputs, HashMap::new());
    assert_eq!(vm.outbox.executions[0].target_type, ExecutionType::Workflow);
}

#[test]
fn test_workflow_run_multiple_calls() {
    // Create two workflows in sequence
    let source = r#"
            Workflow.run("first_workflow", Inputs)
            return Workflow.run("second_workflow", Inputs)
        "#;

    let mut env = HashMap::new();
    env.insert("value".to_string(), Val::Num(123.0));

    let mut vm = parse_workflow_and_build_vm(source, env);
    run_until_done(&mut vm);

    // Check outbox has two execution creations
    assert_eq!(vm.outbox.executions.len(), 2);
    assert_eq!(vm.outbox.executions[0].target_name, "first_workflow");
    assert_eq!(vm.outbox.executions[0].target_type, ExecutionType::Workflow);
    assert_eq!(vm.outbox.executions[1].target_name, "second_workflow");
    assert_eq!(vm.outbox.executions[1].target_type, ExecutionType::Workflow);

    // Execution IDs should be different
    assert_ne!(vm.outbox.executions[0].id, vm.outbox.executions[1].id);
}

#[test]
fn test_workflow_fire_and_forget_then_await() {
    // Fire-and-forget one workflow, then await another
    let source = r#"
            let inputs1 = { background: true }
            let inputs2 = { foreground: true }
            Workflow.run("fire_and_forget_workflow", inputs1)
            return await Workflow.run("awaited_workflow", inputs2)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());

    run_until_done(&mut vm);

    // VM should be suspended on the awaited workflow
    match &vm.control {
        Control::Suspend(Awaitable::Execution(exec_id)) => {
            assert_eq!(exec_id.len(), 36);
        }
        _ => panic!(
            "Expected Control::Suspend(Awaitable::Execution(_)), got {:?}",
            vm.control
        ),
    }

    // Outbox should contain BOTH executions
    assert_eq!(vm.outbox.executions.len(), 2);

    // First execution (fire-and-forget)
    assert_eq!(
        vm.outbox.executions[0].target_name,
        "fire_and_forget_workflow"
    );
    let mut expected_inputs1 = HashMap::new();
    expected_inputs1.insert("background".to_string(), Val::Bool(true));
    assert_eq!(vm.outbox.executions[0].inputs, expected_inputs1);
    assert_eq!(vm.outbox.executions[0].target_type, ExecutionType::Workflow);

    // Second execution (awaited)
    assert_eq!(vm.outbox.executions[1].target_name, "awaited_workflow");
    let mut expected_inputs2 = HashMap::new();
    expected_inputs2.insert("foreground".to_string(), Val::Bool(true));
    assert_eq!(vm.outbox.executions[1].inputs, expected_inputs2);
    assert_eq!(vm.outbox.executions[1].target_type, ExecutionType::Workflow);

    // The suspended execution ID should match the second execution in the outbox
    if let Control::Suspend(Awaitable::Execution(suspended_id)) = &vm.control {
        assert_eq!(vm.outbox.executions[1].id, *suspended_id);
    }

    // Execution IDs should be different
    assert_ne!(vm.outbox.executions[0].id, vm.outbox.executions[1].id);
}

#[test]
fn test_workflow_await_and_resume() {
    // Await a workflow and resume with a result
    let source = r#"
            result = await Workflow.run("child_workflow", {})
            return result
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should suspend on the workflow
    assert!(matches!(
        vm.control,
        Control::Suspend(Awaitable::Execution(_))
    ));

    // Resume with a result
    let workflow_result = Val::Obj(
        [
            ("status".to_string(), Val::Str("complete".to_string())),
            ("count".to_string(), Val::Num(42.0)),
        ]
        .into_iter()
        .collect(),
    );
    assert!(vm.resume(workflow_result.clone()));

    // Continue execution
    run_until_done(&mut vm);

    // Should return the resumed value
    assert_eq!(vm.control, Control::Return(workflow_result));
}

/* ===================== Mixed Task and Workflow Tests ===================== */

#[test]
fn test_mixed_task_and_workflow() {
    // Create both a task and a workflow
    let source = r#"
            Task.run("my_task", {})
            return Workflow.run("my_workflow", {})
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should have two executions in outbox
    assert_eq!(vm.outbox.executions.len(), 2);

    // First is a task
    assert_eq!(vm.outbox.executions[0].target_name, "my_task");
    assert_eq!(vm.outbox.executions[0].target_type, ExecutionType::Task);

    // Second is a workflow
    assert_eq!(vm.outbox.executions[1].target_name, "my_workflow");
    assert_eq!(vm.outbox.executions[1].target_type, ExecutionType::Workflow);
}

#[test]
fn test_await_task_then_workflow() {
    // Await a task, then await a workflow
    let source = r#"
            task_result = await Task.run("my_task", {})
            workflow_result = await Workflow.run("my_workflow", {value: task_result})
            return workflow_result
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should suspend on the task first
    assert!(matches!(
        vm.control,
        Control::Suspend(Awaitable::Execution(_))
    ));

    // First execution should be a task
    assert_eq!(vm.outbox.executions.len(), 1);
    assert_eq!(vm.outbox.executions[0].target_type, ExecutionType::Task);

    // Resume with task result
    vm.resume(Val::Num(100.0));
    run_until_done(&mut vm);

    // Now should suspend on the workflow
    assert!(matches!(
        vm.control,
        Control::Suspend(Awaitable::Execution(_))
    ));

    // Should now have two executions
    assert_eq!(vm.outbox.executions.len(), 2);
    assert_eq!(vm.outbox.executions[1].target_type, ExecutionType::Workflow);

    // Workflow inputs should contain the task result
    assert_eq!(
        vm.outbox.executions[1].inputs.get("value"),
        Some(&Val::Num(100.0))
    );

    // Resume with workflow result
    vm.resume(Val::Str("done".to_string()));
    run_until_done(&mut vm);

    // Should return the workflow result
    assert_eq!(vm.control, Control::Return(Val::Str("done".to_string())));
}

/* ===================== Workflow.run() Error Tests ===================== */

#[test]
fn test_workflow_run_wrong_arg_count_one_arg() {
    // Workflow.run("my_workflow") - missing inputs argument
    let source = r#"
            return Workflow.run("my_workflow")
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
fn test_workflow_run_wrong_arg_count_three_args() {
    // Workflow.run("my_workflow", {}, extra) - too many arguments
    let source = r#"
            obj = {}
            return Workflow.run("my_workflow", obj, 42)
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
fn test_workflow_run_first_arg_not_string() {
    // Workflow.run(42, {}) - workflow_name must be string
    let source = r#"
            obj = {}
            return Workflow.run(42, obj)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw WRONG_ARG_TYPE error
    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_TYPE);
    assert!(err.message.contains("workflow_name"));
    assert!(err.message.contains("string"));
}

#[test]
fn test_workflow_run_second_arg_not_object() {
    // Workflow.run("my_workflow", 42) - inputs must be object
    let source = r#"
            return Workflow.run("my_workflow", 42)
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
