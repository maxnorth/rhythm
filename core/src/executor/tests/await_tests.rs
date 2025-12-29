//! Tests for await/suspend/resume functionality
//!
//! Tests for await expressions, suspension, and resumption

use super::helpers::parse_workflow_and_build_vm;
use crate::executor::{run_until_done, Awaitable, Control, Val, VM};
use maplit::hashmap;
use std::collections::HashMap;

#[test]
fn test_await_suspend_basic() {
    // Test that awaiting a Promise value suspends execution
    let source = r#"
            return await Inputs.task
        "#;

    let inputs = hashmap! {
        "task".to_string() => Val::Promise(Awaitable::Task("task-123".to_string())),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    // Should suspend on the task
    assert_eq!(
        vm.control,
        Control::Suspend(Awaitable::Task("task-123".to_string()))
    );

    // Frame should still be on the stack (not popped)
    assert_eq!(vm.frames.len(), 2); // Block + Return frames
}

#[test]
fn test_await_resume() {
    // Test that we can resume after suspension and get the result
    let source = r#"
            return await Inputs.task
        "#;

    let inputs = hashmap! {
        "task".to_string() => Val::Promise(Awaitable::Task("task-123".to_string())),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    // Should suspend on the task
    assert_eq!(
        vm.control,
        Control::Suspend(Awaitable::Task("task-123".to_string()))
    );

    // Serialize the suspended VM
    let serialized = serde_json::to_string(&vm).unwrap();

    // Deserialize it back
    let mut vm2: VM = serde_json::from_str(&serialized).unwrap();

    // Should still be suspended
    assert_eq!(
        vm2.control,
        Control::Suspend(Awaitable::Task("task-123".to_string()))
    );

    // Resume with a result
    let result = Val::Str("task result".to_string());
    assert!(vm2.resume(result.clone()));

    // Control should be cleared
    assert_eq!(vm2.control, Control::None);

    // Continue execution
    run_until_done(&mut vm2);

    // Should return the resumed value
    assert_eq!(vm2.control, Control::Return(result));
}

#[test]
fn test_await_resume_with_num() {
    // Test resuming with a number result (with serialization)
    let source = r#"
            return await Inputs.task
        "#;

    let inputs = hashmap! {
        "task".to_string() => Val::Promise(Awaitable::Task("task-456".to_string())),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Suspend(Awaitable::Task("task-456".to_string()))
    );

    // Serialize and deserialize
    let serialized = serde_json::to_string(&vm).unwrap();
    let mut vm2: VM = serde_json::from_str(&serialized).unwrap();

    // Resume with a numeric result
    assert!(vm2.resume(Val::Num(42.0)));
    run_until_done(&mut vm2);

    assert_eq!(vm2.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_await_non_task_idempotent() {
    // Test that awaiting a non-Task value just returns that value (like JS)
    let source = r#"
            return await 42
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // Should NOT suspend - should just return the number
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_await_non_task_string() {
    // Test awaiting a string value
    let source = r#"
            return await "hello"
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // Should NOT suspend
    assert_eq!(vm.control, Control::Return(Val::Str("hello".to_string())));
}

#[test]
fn test_resume_when_not_suspended_fails() {
    // Test that calling resume when not suspended returns false
    let source = r#"
            return 42
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // VM is not suspended, so resume should fail
    assert!(!vm.resume(Val::Num(100.0)));

    // Control should still be Return (unchanged)
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_await_preserves_frames() {
    // Test that suspension preserves the full frame stack (with serialization)
    // Uses nested blocks to create multiple frames
    let source = r#"
            {
                {
                    return await Inputs.task
                }
            }
        "#;

    let inputs = hashmap! {
        "task".to_string() => Val::Promise(Awaitable::Task("task-789".to_string())),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    // Should suspend
    assert_eq!(
        vm.control,
        Control::Suspend(Awaitable::Task("task-789".to_string()))
    );

    // Should have 4 frames: workflow body Block, outer nested Block, inner nested Block, Return
    assert_eq!(vm.frames.len(), 4);

    // Serialize and deserialize
    let serialized = serde_json::to_string(&vm).unwrap();
    let mut vm2: VM = serde_json::from_str(&serialized).unwrap();

    // Frames should be preserved
    assert_eq!(vm2.frames.len(), 4);

    // Resume and finish
    assert!(vm2.resume(Val::Bool(true)));
    run_until_done(&mut vm2);

    // Should return the resumed value
    assert_eq!(vm2.control, Control::Return(Val::Bool(true)));

    // Frames should be cleared after completion
    assert_eq!(vm2.frames.len(), 0);
}

#[test]
fn test_serialization_with_suspend() {
    // Test that a suspended VM can be serialized and deserialized
    let source = r#"
            return await Inputs.task
        "#;

    let inputs = hashmap! {
        "task".to_string() => Val::Promise(Awaitable::Task("task-serial".to_string())),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    // Should suspend
    assert_eq!(
        vm.control,
        Control::Suspend(Awaitable::Task("task-serial".to_string()))
    );

    // Serialize the VM
    let serialized = serde_json::to_string(&vm).unwrap();

    // Deserialize it back
    let mut vm2: VM = serde_json::from_str(&serialized).unwrap();

    // Should still be suspended
    assert_eq!(
        vm2.control,
        Control::Suspend(Awaitable::Task("task-serial".to_string()))
    );

    // Resume and finish
    assert!(vm2.resume(Val::Num(99.0)));
    run_until_done(&mut vm2);

    // Should return the resumed value
    assert_eq!(vm2.control, Control::Return(Val::Num(99.0)));
}

#[test]
fn test_step_by_step_suspension() {
    // Test suspension, serialization, and resumption
    let source = r#"
            return await Inputs.task
        "#;

    let inputs = hashmap! {
        "task".to_string() => Val::Promise(Awaitable::Task("task-step".to_string())),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    // Should have suspended
    assert_eq!(
        vm.control,
        Control::Suspend(Awaitable::Task("task-step".to_string()))
    );

    // Serialize and deserialize
    let serialized = serde_json::to_string(&vm).unwrap();
    let mut vm2: VM = serde_json::from_str(&serialized).unwrap();

    // Resume and complete
    assert!(vm2.resume(Val::Str("stepped".to_string())));
    run_until_done(&mut vm2);

    assert_eq!(
        vm2.control,
        Control::Return(Val::Str("stepped".to_string()))
    );
}

#[test]
fn test_await_task_run_with_multiline_object() {
    // Test await Task.run() with multiline object argument
    let source = r#"
        let result = await Task.run("processOrder", {
            orderId: 123,
            userId: 456,
            total: 99.99,
            items: ["item1", "item2"]
        })
        return result
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should suspend on the task
    let Control::Suspend(Awaitable::Task(task_id)) = &vm.control else {
        panic!("Expected Suspend on Task, got {:?}", vm.control);
    };

    // Check outbox has the task with correct inputs
    assert_eq!(vm.outbox.tasks.len(), 1);
    assert_eq!(vm.outbox.tasks[0].task_name, "processOrder");
    assert_eq!(&vm.outbox.tasks[0].task_id, task_id);

    let inputs = &vm.outbox.tasks[0].inputs;
    assert_eq!(inputs.get("orderId").unwrap(), &Val::Num(123.0));
    assert_eq!(inputs.get("userId").unwrap(), &Val::Num(456.0));
    assert_eq!(inputs.get("total").unwrap(), &Val::Num(99.99));

    let items = inputs.get("items").unwrap();
    assert_eq!(
        items,
        &Val::List(vec![
            Val::Str("item1".to_string()),
            Val::Str("item2".to_string())
        ])
    );

    // Resume with a result
    vm.resume(Val::Str("order_processed".to_string()));
    run_until_done(&mut vm);

    // Should return the resumed value
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("order_processed".to_string()))
    );
}
