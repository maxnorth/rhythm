//! Tests for standard library functions
//!
//! Tests function calls and Math stdlib

use super::helpers::parse_workflow_and_build_vm;
use crate::executor::{errors, run_until_done, Control, Stmt, Val, VM};
use maplit::hashmap;
use std::collections::HashMap;

/* ===================== Math.floor Tests ===================== */

#[test]
fn test_math_floor_basic() {
    let source = r#"
            return Math.floor(3.7)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}

#[test]
fn test_math_floor_negative() {
    let source = r#"
            return Math.floor(-3.2)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(-4.0)));
}

/* ===================== Math.ceil Tests ===================== */

#[test]
fn test_math_ceil_basic() {
    let source = r#"
            return Math.ceil(3.2)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(4.0)));
}

/* ===================== Math.abs Tests ===================== */

#[test]
fn test_math_abs_positive() {
    let source = r#"
            return Math.abs(5.0)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(5.0)));
}

#[test]
fn test_math_abs_negative() {
    let source = r#"
            return Math.abs(-5.0)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(5.0)));
}

/* ===================== Math.round Tests ===================== */

#[test]
fn test_math_round_basic() {
    let source = r#"
            return Math.round(3.6)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(4.0)));
}

#[test]
fn test_math_round_half() {
    // JavaScript rounds 2.5 to 3 (half-way cases round towards +∞)
    let source = r#"
            return Math.round(2.5)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}

/* ===================== Error Tests ===================== */

#[test]
fn test_call_not_a_function() {
    // Try to call a number - parser doesn't support this yet, use JSON
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "LitNum",
                    "v": 42.0
                },
                "args": []
            }
        }]
        }
        "#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::NOT_A_FUNCTION);
    assert!(err.message.contains("not callable"));
}

#[test]
fn test_wrong_arg_count() {
    let source = r#"
            return Math.floor()
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_COUNT);
    assert!(err.message.contains("Expected 1 argument"));
}

#[test]
fn test_wrong_arg_type() {
    let source = r#"
            return Math.floor("not a number")
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_TYPE);
    assert!(err.message.contains("must be a number"));
}

/* ===================== Nested/Complex Tests ===================== */

#[test]
fn test_nested_call() {
    // Math.abs(Math.floor(-3.7)) should return 4.0
    let source = r#"
            return Math.abs(Math.floor(-3.7))
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(4.0)));
}

#[test]
fn test_call_with_member_chain() {
    let source = r#"
            return Math.floor(Inputs.value)
        "#;

    let inputs = hashmap! {
        "value".to_string() => Val::Num(3.7),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}
