//! Tests for Task.run() and outbox functionality

use crate::interpreter::executor_v2::{errors, run_until_done, Control, Stmt, Val, VM};
use std::collections::HashMap;

/* ===================== Task.run() Tests ===================== */

#[test]
fn test_task_run_basic() {
    // Task.run("my_task", {input: 42})
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Task"
                    },
                    "property": "run"
                },
                "args": [{
                    "t": "LitStr",
                    "v": "my_task"
                }, {
                    "t": "Ident",
                    "name": "inputs"
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut inputs_obj = HashMap::new();
    inputs_obj.insert("input".to_string(), Val::Num(42.0));

    let mut env = HashMap::new();
    env.insert("inputs".to_string(), Val::Obj(inputs_obj.clone()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should return a Task value with a UUID
    match &vm.control {
        Control::Return(Val::Task(task_id)) => {
            // Task ID should be a valid UUID format (36 characters with dashes)
            assert_eq!(task_id.len(), 36);
            assert!(task_id.contains('-'));
        }
        _ => panic!("Expected Control::Return(Val::Task(_)), got {:?}", vm.control),
    }

    // Check outbox has one task creation
    assert_eq!(vm.outbox.len(), 1);
    let task_creation = &vm.outbox[0];
    assert_eq!(task_creation.task_name, "my_task");
    assert_eq!(task_creation.inputs, inputs_obj);

    // Task ID in outbox should match task ID in return value
    if let Control::Return(Val::Task(task_id)) = &vm.control {
        assert_eq!(task_creation.task_id, *task_id);
    }
}

#[test]
fn test_task_run_empty_inputs() {
    // Task.run("simple_task", {})
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Task"
                    },
                    "property": "run"
                },
                "args": [{
                    "t": "LitStr",
                    "v": "simple_task"
                }, {
                    "t": "Ident",
                    "name": "empty"
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("empty".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should return a Task value
    assert!(matches!(vm.control, Control::Return(Val::Task(_))));

    // Check outbox
    assert_eq!(vm.outbox.len(), 1);
    assert_eq!(vm.outbox[0].task_name, "simple_task");
    assert_eq!(vm.outbox[0].inputs, HashMap::new());
}

#[test]
fn test_task_run_multiple_calls() {
    // Create two tasks in sequence
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Expr",
                "expr": {
                    "t": "Call",
                    "callee": {
                        "t": "Member",
                        "object": {
                            "t": "Ident",
                            "name": "Task"
                        },
                        "property": "run"
                    },
                    "args": [{
                        "t": "LitStr",
                        "v": "first_task"
                    }, {
                        "t": "Ident",
                        "name": "inputs"
                    }]
                }
            },
            {
                "t": "Return",
                "value": {
                    "t": "Call",
                    "callee": {
                        "t": "Member",
                        "object": {
                            "t": "Ident",
                            "name": "Task"
                        },
                        "property": "run"
                    },
                    "args": [{
                        "t": "LitStr",
                        "v": "second_task"
                    }, {
                        "t": "Ident",
                        "name": "inputs"
                    }]
                }
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut inputs_obj = HashMap::new();
    inputs_obj.insert("value".to_string(), Val::Num(123.0));

    let mut env = HashMap::new();
    env.insert("inputs".to_string(), Val::Obj(inputs_obj));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Check outbox has two task creations
    assert_eq!(vm.outbox.len(), 2);
    assert_eq!(vm.outbox[0].task_name, "first_task");
    assert_eq!(vm.outbox[1].task_name, "second_task");

    // Task IDs should be different
    assert_ne!(vm.outbox[0].task_id, vm.outbox[1].task_id);
}

#[test]
fn test_fire_and_forget_then_await() {
    // Fire-and-forget one task, then await another
    // This tests that:
    // 1. Fire-and-forget tasks are recorded in the outbox
    // 2. Awaited tasks also get recorded in the outbox
    // 3. VM suspends on the await, preserving state
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Expr",
                "expr": {
                    "t": "Call",
                    "callee": {
                        "t": "Member",
                        "object": {
                            "t": "Ident",
                            "name": "Task"
                        },
                        "property": "run"
                    },
                    "args": [{
                        "t": "LitStr",
                        "v": "fire_and_forget_task"
                    }, {
                        "t": "Ident",
                        "name": "inputs1"
                    }]
                }
            },
            {
                "t": "Return",
                "value": {
                    "t": "Await",
                    "inner": {
                        "t": "Call",
                        "callee": {
                            "t": "Member",
                            "object": {
                                "t": "Ident",
                                "name": "Task"
                            },
                            "property": "run"
                        },
                        "args": [{
                            "t": "LitStr",
                            "v": "awaited_task"
                        }, {
                            "t": "Ident",
                            "name": "inputs2"
                        }]
                    }
                }
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut inputs1 = HashMap::new();
    inputs1.insert("background".to_string(), Val::Bool(true));

    let mut inputs2 = HashMap::new();
    inputs2.insert("foreground".to_string(), Val::Bool(true));

    let mut env = HashMap::new();
    env.insert("inputs1".to_string(), Val::Obj(inputs1.clone()));
    env.insert("inputs2".to_string(), Val::Obj(inputs2.clone()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // VM should be suspended on the awaited task
    match &vm.control {
        Control::Suspend(task_id) => {
            // Should be suspended on the second task (the awaited one)
            assert_eq!(task_id.len(), 36);
        }
        _ => panic!("Expected Control::Suspend, got {:?}", vm.control),
    }

    // Outbox should contain BOTH tasks
    assert_eq!(vm.outbox.len(), 2);

    // First task (fire-and-forget)
    assert_eq!(vm.outbox[0].task_name, "fire_and_forget_task");
    assert_eq!(vm.outbox[0].inputs, inputs1);

    // Second task (awaited)
    assert_eq!(vm.outbox[1].task_name, "awaited_task");
    assert_eq!(vm.outbox[1].inputs, inputs2);

    // The suspended task ID should match the second task in the outbox
    if let Control::Suspend(suspended_id) = &vm.control {
        assert_eq!(vm.outbox[1].task_id, *suspended_id);
    }

    // Task IDs should be different
    assert_ne!(vm.outbox[0].task_id, vm.outbox[1].task_id);

    // Frames should be preserved (not popped due to suspension)
    assert_eq!(vm.frames.len(), 2); // Block + Return frames
}

/* ===================== Task.run() Error Tests ===================== */

#[test]
fn test_task_run_wrong_arg_count_one_arg() {
    // Task.run("my_task") - missing inputs argument
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Task"
                    },
                    "property": "run"
                },
                "args": [{
                    "t": "LitStr",
                    "v": "my_task"
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
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
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Task"
                    },
                    "property": "run"
                },
                "args": [{
                    "t": "LitStr",
                    "v": "my_task"
                }, {
                    "t": "Ident",
                    "name": "obj"
                }, {
                    "t": "LitNum",
                    "v": 42.0
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut env = HashMap::new();
    env.insert("obj".to_string(), Val::Obj(HashMap::new()));
    let mut vm = VM::new(program, env);
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
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Task"
                    },
                    "property": "run"
                },
                "args": [{
                    "t": "LitNum",
                    "v": 42.0
                }, {
                    "t": "Ident",
                    "name": "obj"
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut env = HashMap::new();
    env.insert("obj".to_string(), Val::Obj(HashMap::new()));
    let mut vm = VM::new(program, env);
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
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Task"
                    },
                    "property": "run"
                },
                "args": [{
                    "t": "LitStr",
                    "v": "my_task"
                }, {
                    "t": "LitNum",
                    "v": 42.0
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    // Should throw WRONG_ARG_TYPE error
    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_TYPE);
    assert!(err.message.contains("inputs"));
    assert!(err.message.contains("object"));
}
