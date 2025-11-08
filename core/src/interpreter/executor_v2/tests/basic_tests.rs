//! Basic tests for core execution loop
//!
//! Tests for Milestone 1: Return statement with literal expressions

use crate::interpreter::executor_v2::{errors, run_until_done, Control, Stmt, Val, VM};
use std::collections::HashMap;

#[test]
fn test_return_literal_num() {
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

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_return_literal_bool() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "LitBool",
                "v": true
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_return_literal_str() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "LitStr",
                "v": "hello"
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::Str("hello".to_string()))
    );
}

#[test]
fn test_return_null() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": null
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Null));
}

#[test]
fn test_nested_blocks() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Block",
            "body": [{
                "t": "Block",
                "body": [{
                    "t": "Return",
                    "value": {
                        "t": "LitNum",
                        "v": 42.0
                    }
                }]
            }]
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_return_ctx() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Ident",
                "name": "ctx"
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    // Set up initial environment with ctx
    let mut env = HashMap::new();
    env.insert("ctx".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // ctx should be an empty object
    assert_eq!(vm.control, Control::Return(Val::Obj(HashMap::new())));
}

#[test]
fn test_return_inputs() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Ident",
                "name": "inputs"
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    // Set up initial environment with inputs
    let mut env = HashMap::new();
    env.insert("inputs".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // inputs should be an empty object
    assert_eq!(
        vm.control,
        Control::Return(Val::Obj(HashMap::new()))
    );
}

#[test]
fn test_initial_env() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Ident",
                "name": "inputs"
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    // Set up initial environment with whatever variables we want
    let mut inputs_obj = HashMap::new();
    inputs_obj.insert("name".to_string(), Val::Str("Alice".to_string()));
    inputs_obj.insert("age".to_string(), Val::Num(30.0));

    let mut env = HashMap::new();
    env.insert("inputs".to_string(), Val::Obj(inputs_obj.clone()));
    env.insert("ctx".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should return the inputs object we provided
    assert_eq!(vm.control, Control::Return(Val::Obj(inputs_obj)));
}

#[test]
fn test_member_access() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Member",
                "object": {
                    "t": "Ident",
                    "name": "inputs"
                },
                "property": "name"
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    // Set up initial environment with inputs containing properties
    let mut inputs_obj = HashMap::new();
    inputs_obj.insert("name".to_string(), Val::Str("Alice".to_string()));
    inputs_obj.insert("age".to_string(), Val::Num(30.0));

    let mut env = HashMap::new();
    env.insert("inputs".to_string(), Val::Obj(inputs_obj));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should return inputs.name
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("Alice".to_string()))
    );
}

#[test]
fn test_nested_member_access() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Member",
                "object": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "ctx"
                    },
                    "property": "user"
                },
                "property": "id"
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    // Set up nested object: ctx.user.id
    let mut user_obj = HashMap::new();
    user_obj.insert("id".to_string(), Val::Num(123.0));
    user_obj.insert("name".to_string(), Val::Str("Bob".to_string()));

    let mut ctx_obj = HashMap::new();
    ctx_obj.insert("user".to_string(), Val::Obj(user_obj));

    let mut env = HashMap::new();
    env.insert("ctx".to_string(), Val::Obj(ctx_obj));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should return ctx.user.id
    assert_eq!(vm.control, Control::Return(Val::Num(123.0)));
}

/* ===================== Expression Statement Tests ===================== */

#[test]
fn test_expr_stmt_simple() {
    // Test a simple expression statement (value is discarded)
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Expr",
                "expr": {
                    "t": "LitNum",
                    "v": 42.0
                }
            },
            {
                "t": "Return",
                "value": {
                    "t": "LitStr",
                    "v": "done"
                }
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    // Should return "done" (the expr statement result is discarded)
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("done".to_string()))
    );
}

#[test]
fn test_expr_stmt_with_member_access() {
    // Test expression statement with member access
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Expr",
                "expr": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "obj"
                    },
                    "property": "value"
                }
            },
            {
                "t": "Return",
                "value": {
                    "t": "LitNum",
                    "v": 999.0
                }
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut obj = HashMap::new();
    obj.insert("value".to_string(), Val::Str("ignored".to_string()));

    let mut env = HashMap::new();
    env.insert("obj".to_string(), Val::Obj(obj));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should return 999 (the expr statement result is discarded)
    assert_eq!(vm.control, Control::Return(Val::Num(999.0)));
}

#[test]
fn test_expr_stmt_error_propagates() {
    // Test that errors in expression statements propagate correctly
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Expr",
                "expr": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "obj"
                    },
                    "property": "missing"
                }
            },
            {
                "t": "Return",
                "value": {
                    "t": "LitStr",
                    "v": "should_not_reach"
                }
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("obj".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should throw an error
    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::PROPERTY_NOT_FOUND);
    assert!(err.message.contains("Property 'missing' not found"));
}
