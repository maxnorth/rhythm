//! Tests for await/suspend/resume functionality
//!
//! Tests for await expressions, suspension, and resumption

use crate::interpreter::executor_v2::{run_until_done, step, Control, Step, Stmt, Val, VM};
use std::collections::HashMap;

#[test]
fn test_await_suspend_basic() {
    // Test that awaiting a Task value suspends execution
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Await",
                "inner": {
                    "t": "Ident",
                    "name": "task"
                }
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    // Set up environment with a Task value
    let mut env = HashMap::new();
    env.insert("task".to_string(), Val::Task("task-123".to_string()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should suspend on the task
    assert_eq!(vm.control, Control::Suspend("task-123".to_string()));

    // Frame should still be on the stack (not popped)
    assert_eq!(vm.frames.len(), 2); // Block + Return frames
}

#[test]
fn test_await_resume() {
    // Test that we can resume after suspension and get the result
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Await",
                "inner": {
                    "t": "Ident",
                    "name": "task"
                }
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    // Set up environment with a Task value
    let mut env = HashMap::new();
    env.insert("task".to_string(), Val::Task("task-123".to_string()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should suspend on the task
    assert_eq!(vm.control, Control::Suspend("task-123".to_string()));

    // Serialize the suspended VM
    let serialized = serde_json::to_string(&vm).unwrap();

    // Deserialize it back
    let mut vm2: VM = serde_json::from_str(&serialized).unwrap();

    // Should still be suspended
    assert_eq!(vm2.control, Control::Suspend("task-123".to_string()));

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
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Await",
                "inner": {
                    "t": "Ident",
                    "name": "task"
                }
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("task".to_string(), Val::Task("task-456".to_string()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Suspend("task-456".to_string()));

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
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Await",
                "inner": {
                    "t": "LitNum",
                    "v": 42.0
                }
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    // Should NOT suspend - should just return the number
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_await_non_task_string() {
    // Test awaiting a string value
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Await",
                "inner": {
                    "t": "LitStr",
                    "v": "hello"
                }
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    // Should NOT suspend
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("hello".to_string()))
    );
}

#[test]
fn test_resume_when_not_suspended_fails() {
    // Test that calling resume when not suspended returns false
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "LitNum",
                "v": 42.0
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    // VM is not suspended, so resume should fail
    assert!(!vm.resume(Val::Num(100.0)));

    // Control should still be Return (unchanged)
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_await_preserves_frames() {
    // Test that suspension preserves the full frame stack (with serialization)
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Block",
            "body": [{
                "t": "Return",
                "value": {
                    "t": "Await",
                    "inner": {
                        "t": "Ident",
                        "name": "task"
                    }
                }
            }]
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("task".to_string(), Val::Task("task-789".to_string()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should suspend
    assert_eq!(vm.control, Control::Suspend("task-789".to_string()));

    // Should have 3 frames: outer Block, inner Block, Return
    assert_eq!(vm.frames.len(), 3);

    // Serialize and deserialize
    let serialized = serde_json::to_string(&vm).unwrap();
    let mut vm2: VM = serde_json::from_str(&serialized).unwrap();

    // Frames should be preserved
    assert_eq!(vm2.frames.len(), 3);

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
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Await",
                "inner": {
                    "t": "Ident",
                    "name": "task"
                }
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("task".to_string(), Val::Task("task-serial".to_string()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should suspend
    assert_eq!(vm.control, Control::Suspend("task-serial".to_string()));

    // Serialize the VM
    let serialized = serde_json::to_string(&vm).unwrap();

    // Deserialize it back
    let mut vm2: VM = serde_json::from_str(&serialized).unwrap();

    // Should still be suspended
    assert_eq!(vm2.control, Control::Suspend("task-serial".to_string()));

    // Resume and finish
    assert!(vm2.resume(Val::Num(99.0)));
    run_until_done(&mut vm2);

    // Should return the resumed value
    assert_eq!(vm2.control, Control::Return(Val::Num(99.0)));
}

#[test]
fn test_step_by_step_suspension() {
    // Test stepping through suspension manually
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Await",
                "inner": {
                    "t": "Ident",
                    "name": "task"
                }
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("task".to_string(), Val::Task("task-step".to_string()));

    let mut vm = VM::new(program, env);

    // Step through execution manually
    let mut step_count = 0;
    loop {
        match step(&mut vm) {
            Step::Continue => {
                step_count += 1;
            }
            Step::Done => break,
        }
    }

    // Should have suspended
    assert_eq!(vm.control, Control::Suspend("task-step".to_string()));
    assert!(step_count > 0);

    // Serialize and deserialize
    let serialized = serde_json::to_string(&vm).unwrap();
    let mut vm2: VM = serde_json::from_str(&serialized).unwrap();

    // Resume
    assert!(vm2.resume(Val::Str("stepped".to_string())));

    // Step through completion
    loop {
        match step(&mut vm2) {
            Step::Continue => {}
            Step::Done => break,
        }
    }

    assert_eq!(
        vm2.control,
        Control::Return(Val::Str("stepped".to_string()))
    );
}
