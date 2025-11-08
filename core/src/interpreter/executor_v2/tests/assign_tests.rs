//! Tests for assignment statements

use crate::interpreter::executor_v2::{run_until_done, Control, Stmt, Val, VM};
use std::collections::HashMap;

/* ===================== Basic Assignment Tests ===================== */

#[test]
fn test_assign_number() {
    // x = 42; return x;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "name": "x",
                "expr": {"t": "LitNum", "v": 42.0}
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "x"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
    assert_eq!(vm.env.get("x"), Some(&Val::Num(42.0)));
}

#[test]
fn test_assign_string() {
    // name = "Alice"; return name;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "name": "name",
                "expr": {"t": "LitStr", "v": "Alice"}
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "name"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::Str("Alice".to_string()))
    );
    assert_eq!(vm.env.get("name"), Some(&Val::Str("Alice".to_string())));
}

#[test]
fn test_assign_array() {
    // items = [1, 2, 3]; return items;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "name": "items",
                "expr": {
                    "t": "LitList",
                    "elements": [
                        {"t": "LitNum", "v": 1.0},
                        {"t": "LitNum", "v": 2.0},
                        {"t": "LitNum", "v": 3.0}
                    ]
                }
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "items"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    let expected = Val::List(vec![Val::Num(1.0), Val::Num(2.0), Val::Num(3.0)]);
    assert_eq!(vm.control, Control::Return(expected.clone()));
    assert_eq!(vm.env.get("items"), Some(&expected));
}

#[test]
fn test_assign_object() {
    // user = {name: "Bob", age: 30}; return user;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "name": "user",
                "expr": {
                    "t": "LitObj",
                    "properties": [
                        ["name", {"t": "LitStr", "v": "Bob"}],
                        ["age", {"t": "LitNum", "v": 30.0}]
                    ]
                }
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "user"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    let mut expected_map = HashMap::new();
    expected_map.insert("name".to_string(), Val::Str("Bob".to_string()));
    expected_map.insert("age".to_string(), Val::Num(30.0));
    let expected = Val::Obj(expected_map);

    assert_eq!(vm.control, Control::Return(expected.clone()));
    assert_eq!(vm.env.get("user"), Some(&expected));
}

/* ===================== Assignment with Expressions ===================== */

#[test]
fn test_assign_from_variable() {
    // y = x; return y;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "name": "y",
                "expr": {"t": "Ident", "name": "x"}
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "y"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("x".to_string(), Val::Num(100.0));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(100.0)));
    assert_eq!(vm.env.get("y"), Some(&Val::Num(100.0)));
    assert_eq!(vm.env.get("x"), Some(&Val::Num(100.0))); // x unchanged
}

#[test]
fn test_assign_with_member_access() {
    // name = ctx.user; return name;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "name": "name",
                "expr": {
                    "t": "Member",
                    "object": {"t": "Ident", "name": "ctx"},
                    "property": "user"
                }
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "name"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut ctx_obj = HashMap::new();
    ctx_obj.insert("user".to_string(), Val::Str("Alice".to_string()));

    let mut env = HashMap::new();
    env.insert("ctx".to_string(), Val::Obj(ctx_obj));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::Str("Alice".to_string()))
    );
    assert_eq!(vm.env.get("name"), Some(&Val::Str("Alice".to_string())));
}

#[test]
fn test_assign_with_function_call() {
    // result = Math.abs(x); return result;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "name": "result",
                "expr": {
                    "t": "Call",
                    "callee": {
                        "t": "Member",
                        "object": {"t": "Ident", "name": "Math"},
                        "property": "abs"
                    },
                    "args": [{"t": "Ident", "name": "x"}]
                }
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "result"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("x".to_string(), Val::Num(-42.0));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
    assert_eq!(vm.env.get("result"), Some(&Val::Num(42.0)));
}

/* ===================== Reassignment Tests ===================== */

#[test]
fn test_reassignment() {
    // x = 10; x = 20; return x;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "name": "x",
                "expr": {"t": "LitNum", "v": 10.0}
            },
            {
                "t": "Assign",
                "name": "x",
                "expr": {"t": "LitNum", "v": 20.0}
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "x"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(20.0)));
    assert_eq!(vm.env.get("x"), Some(&Val::Num(20.0)));
}

#[test]
fn test_reassignment_different_type() {
    // x = 42; x = "hello"; return x;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "name": "x",
                "expr": {"t": "LitNum", "v": 42.0}
            },
            {
                "t": "Assign",
                "name": "x",
                "expr": {"t": "LitStr", "v": "hello"}
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "x"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::Str("hello".to_string()))
    );
    assert_eq!(vm.env.get("x"), Some(&Val::Str("hello".to_string())));
}

/* ===================== Assignment with Await ===================== */

#[test]
fn test_assign_with_await() {
    // result = await Task.run("my_task", {});
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "name": "result",
                "expr": {
                    "t": "Await",
                    "inner": {
                        "t": "Call",
                        "callee": {
                            "t": "Member",
                            "object": {"t": "Ident", "name": "Task"},
                            "property": "run"
                        },
                        "args": [
                            {"t": "LitStr", "v": "my_task"},
                            {"t": "LitObj", "properties": []}
                        ]
                    }
                }
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "result"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    // Should be suspended on the awaited task
    match &vm.control {
        Control::Suspend(task_id) => {
            assert_eq!(task_id.len(), 36); // UUID format
        }
        _ => panic!("Expected Control::Suspend, got {:?}", vm.control),
    }

    // The assignment should NOT have completed yet (variable not in env)
    assert_eq!(vm.env.get("result"), None);

    // Frames should be preserved (not popped due to suspension)
    assert_eq!(vm.frames.len(), 2); // Block + Assign frames
}

#[test]
fn test_assign_with_await_resume() {
    // result = await Task.run("my_task", {}); return result;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "name": "result",
                "expr": {
                    "t": "Await",
                    "inner": {
                        "t": "Call",
                        "callee": {
                            "t": "Member",
                            "object": {"t": "Ident", "name": "Task"},
                            "property": "run"
                        },
                        "args": [
                            {"t": "LitStr", "v": "my_task"},
                            {"t": "LitObj", "properties": []}
                        ]
                    }
                }
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "result"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    // Verify suspension
    assert!(matches!(vm.control, Control::Suspend(_)));

    // Resume with a result value
    let task_result = Val::Num(42.0);
    assert!(vm.resume(task_result.clone()));

    // Run to completion
    run_until_done(&mut vm);

    // Should return the result
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
    // The assignment should have completed
    assert_eq!(vm.env.get("result"), Some(&Val::Num(42.0)));
}

/* ===================== Assignment with Error Handling ===================== */

#[test]
fn test_assign_with_error() {
    // result = ctx.nonexistent;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "name": "result",
                "expr": {
                    "t": "Member",
                    "object": {"t": "Ident", "name": "ctx"},
                    "property": "nonexistent"
                }
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "result"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("ctx".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should throw an error
    match &vm.control {
        Control::Throw(Val::Error(err)) => {
            assert!(err.message.contains("nonexistent"));
        }
        _ => panic!("Expected Control::Throw, got {:?}", vm.control),
    }

    // The assignment should NOT have completed
    assert_eq!(vm.env.get("result"), None);
}

#[test]
fn test_assign_in_try_catch() {
    // try { result = ctx.bad; } catch (e) { result = "error"; } return result;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Try",
                "body": {
                    "t": "Assign",
                    "name": "result",
                    "expr": {
                        "t": "Member",
                        "object": {"t": "Ident", "name": "ctx"},
                        "property": "bad"
                    }
                },
                "catch_var": "e",
                "catch_body": {
                    "t": "Assign",
                    "name": "result",
                    "expr": {"t": "LitStr", "v": "error"}
                }
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "result"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("ctx".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should return "error" from the catch block
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("error".to_string()))
    );
    assert_eq!(vm.env.get("result"), Some(&Val::Str("error".to_string())));
}

/* ===================== Multiple Assignments ===================== */

#[test]
fn test_multiple_assignments() {
    // a = 1; b = 2; c = 3; return [a, b, c];
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "name": "a",
                "expr": {"t": "LitNum", "v": 1.0}
            },
            {
                "t": "Assign",
                "name": "b",
                "expr": {"t": "LitNum", "v": 2.0}
            },
            {
                "t": "Assign",
                "name": "c",
                "expr": {"t": "LitNum", "v": 3.0}
            },
            {
                "t": "Return",
                "value": {
                    "t": "LitList",
                    "elements": [
                        {"t": "Ident", "name": "a"},
                        {"t": "Ident", "name": "b"},
                        {"t": "Ident", "name": "c"}
                    ]
                }
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::List(vec![Val::Num(1.0), Val::Num(2.0), Val::Num(3.0)]))
    );
    assert_eq!(vm.env.get("a"), Some(&Val::Num(1.0)));
    assert_eq!(vm.env.get("b"), Some(&Val::Num(2.0)));
    assert_eq!(vm.env.get("c"), Some(&Val::Num(3.0)));
}
