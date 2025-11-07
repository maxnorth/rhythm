//! Basic tests for core execution loop
//!
//! Tests for Milestone 1: Return statement with literal expressions

use crate::interpreter::executor_v2::{run_until_done, Control, Expr, Stmt, Val, VM};

#[test]
fn test_return_literal_num() {
    let program = Stmt::Block {
        body: vec![Stmt::Return {
            value: Some(Expr::LitNum { v: 42.0 }),
        }],
    };

    let mut vm = VM::new(program);
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

    let mut vm = VM::new(program);
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

    let mut vm = VM::new(program);
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

    let mut vm = VM::new(program);
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(None));
}
