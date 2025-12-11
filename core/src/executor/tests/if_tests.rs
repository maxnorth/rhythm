//! Tests for If statements

use super::super::*;
use super::helpers::parse_workflow_and_build_vm;
use maplit::hashmap;
use std::collections::HashMap;

#[test]
fn test_if_true_no_else() {
    let source = r#"
            if (true) {
                return 42
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_if_false_no_else() {
    let source = r#"
            if (false) {
                return 42
            }
            return 99
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(99.0)));
}

#[test]
fn test_if_true_with_else() {
    let source = r#"
            if (true) {
                return 42
            } else {
                return 99
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_if_false_with_else() {
    let source = r#"
            if (false) {
                return 42
            } else {
                return 99
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(99.0)));
}

#[test]
fn test_if_truthiness_number() {
    // if (42) { return "truthy"; } else { return "falsy"; }
    let source = r#"
            if (42) {
                return "truthy"
            } else {
                return "falsy"
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("truthy".to_string())));
}

#[test]
fn test_if_truthiness_false() {
    // if (false) { return "truthy"; } else { return "falsy"; }
    let source = r#"
            if (false) {
                return "truthy"
            } else {
                return "falsy"
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("falsy".to_string())));
}

#[test]
fn test_if_with_variable_test() {
    // x = true; if (x) { return "yes"; } else { return "no"; }
    let source = r#"
            x = true
            if (x) {
                return "yes"
            } else {
                return "no"
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("yes".to_string())));
}

#[test]
fn test_if_with_assignment_in_branch() {
    // x = 1; if (true) { x = 42; } return x;
    let source = r#"
            x = 1
            if (true) {
                x = 42
            }
            return x
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
    assert_eq!(vm.env.get("x"), Some(&Val::Num(42.0)));
}

#[test]
fn test_if_nested() {
    // if (true) { if (false) { return 1; } else { return 2; } } else { return 3; }
    let source = r#"
            if (true) {
                if (false) {
                    return 1
                } else {
                    return 2
                }
            } else {
                return 3
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(2.0)));
}

#[test]
fn test_if_with_block_statement() {
    // if (true) { x = 1; x = 2; return x; }
    let source = r#"
            if (true) {
                x = 1
                x = 2
                return x
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(2.0)));
}

#[test]
fn test_if_with_error_in_test() {
    // if (Context.bad) { return 1; }
    // ctx doesn't have 'bad' property, so this should throw
    let source = r#"
            if (Context.bad) {
                return 1
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw an error for accessing property that doesn't exist
    match &vm.control {
        Control::Throw(Val::Error(err)) => {
            // Context is defined but doesn't have 'bad' property
            assert_eq!(err.code, errors::PROPERTY_NOT_FOUND);
        }
        _ => panic!("Expected error, got: {:?}", vm.control),
    }
}

#[test]
fn test_if_with_try_catch() {
    // result = "not_set"; if (true) { try { throw {code: "E", message: "msg"}; } catch (e) { result = "caught"; } } return result;
    let source = r#"
            result = "not_set"
            if (true) {
                try {
                    throw({code: "E", message: "msg"})
                } catch (e) {
                    result = "caught"
                }
            }
            return result
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("caught".to_string())));
}

/* ===================== Else-If Chain Tests ===================== */

#[test]
fn test_else_if_chain() {
    // Test else-if chain: if/else if/else
    let source = r#"
            if (Inputs.value) {
                return "first"
            } else if (Inputs.fallback) {
                return "second"
            } else {
                return "third"
            }
    "#;

    // Test first branch
    let inputs = hashmap! {
        "value".to_string() => Val::Bool(true),
        "fallback".to_string() => Val::Bool(false),
    };
    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("first".to_string())));

    // Test second branch (else-if)
    let inputs = hashmap! {
        "value".to_string() => Val::Bool(false),
        "fallback".to_string() => Val::Bool(true),
    };
    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("second".to_string())));

    // Test third branch (else)
    let inputs = hashmap! {
        "value".to_string() => Val::Bool(false),
        "fallback".to_string() => Val::Bool(false),
    };
    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("third".to_string())));
}

#[test]
fn test_multiple_else_if() {
    // Test multiple else-if clauses
    let source = r#"
            if (Inputs.a) {
                return 1
            } else if (Inputs.b) {
                return 2
            } else if (Inputs.c) {
                return 3
            } else {
                return 4
            }
    "#;

    // Test third branch (c)
    let inputs = hashmap! {
        "a".to_string() => Val::Bool(false),
        "b".to_string() => Val::Bool(false),
        "c".to_string() => Val::Bool(true),
    };
    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}
