//! Basic tests for core execution loop
//!
//! Tests for Milestone 1: Return statement with literal expressions

use crate::interpreter::executor_v2::{run_until_done, Control, Expr, Stmt, Val, VM};
use std::collections::HashMap;

#[test]
fn test_return_literal_num() {
    let program = Stmt::Block {
        body: vec![Stmt::Return {
            value: Some(Expr::LitNum { v: 42.0 }),
        }],
    };

    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Some(Val::Num(42.0))));
}

#[test]
fn test_return_literal_bool() {
    let program = Stmt::Block {
        body: vec![Stmt::Return {
            value: Some(Expr::LitBool { v: true }),
        }],
    };

    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Some(Val::Bool(true))));
}

#[test]
fn test_return_literal_str() {
    let program = Stmt::Block {
        body: vec![Stmt::Return {
            value: Some(Expr::LitStr {
                v: "hello".to_string(),
            }),
        }],
    };

    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Some(Val::Str("hello".to_string())))
    );
}

#[test]
fn test_return_unit() {
    let program = Stmt::Block {
        body: vec![Stmt::Return { value: None }],
    };

    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(None));
}

#[test]
fn test_nested_blocks() {
    let program = Stmt::Block {
        body: vec![Stmt::Block {
            body: vec![Stmt::Block {
                body: vec![Stmt::Return {
                    value: Some(Expr::LitNum { v: 42.0 }),
                }],
            }],
        }],
    };

    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Some(Val::Num(42.0))));
}

#[test]
fn test_return_ctx() {
    let program = Stmt::Block {
        body: vec![Stmt::Return {
            value: Some(Expr::Ident {
                name: "ctx".to_string(),
            }),
        }],
    };

    // Set up initial environment with ctx
    let mut env = HashMap::new();
    env.insert("ctx".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // ctx should be an empty object
    assert_eq!(vm.control, Control::Return(Some(Val::Obj(HashMap::new()))));
}

#[test]
fn test_return_inputs() {
    let program = Stmt::Block {
        body: vec![Stmt::Return {
            value: Some(Expr::Ident {
                name: "inputs".to_string(),
            }),
        }],
    };

    // Set up initial environment with inputs
    let mut env = HashMap::new();
    env.insert("inputs".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // inputs should be an empty object
    assert_eq!(
        vm.control,
        Control::Return(Some(Val::Obj(HashMap::new())))
    );
}

#[test]
fn test_initial_env() {
    let program = Stmt::Block {
        body: vec![Stmt::Return {
            value: Some(Expr::Ident {
                name: "inputs".to_string(),
            }),
        }],
    };

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
    assert_eq!(vm.control, Control::Return(Some(Val::Obj(inputs_obj))));
}
