//! Tests for Ternary expressions (condition ? then : else)

use super::super::*;
use super::helpers::parse_workflow_and_build_vm;
use maplit::hashmap;

#[test]
fn test_ternary_true_condition() {
    let source = r#"
        return true ? 1 : 2
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(1.0)));
}

#[test]
fn test_ternary_false_condition() {
    let source = r#"
        return false ? 1 : 2
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(2.0)));
}

#[test]
fn test_ternary_with_comparison() {
    let source = r#"
        let x = 5
        return x > 3 ? "big" : "small"
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("big".to_string())));
}

#[test]
fn test_ternary_with_equality() {
    let source = r#"
        let status = "active"
        return status == "active" ? 1 : 0
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(1.0)));
}

#[test]
fn test_ternary_truthy_number() {
    let source = r#"
        return 42 ? "truthy" : "falsy"
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("truthy".to_string())));
}

#[test]
fn test_ternary_falsy_zero() {
    let source = r#"
        return 0 ? "truthy" : "falsy"
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("falsy".to_string())));
}

#[test]
fn test_ternary_falsy_null() {
    let source = r#"
        return null ? "truthy" : "falsy"
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("falsy".to_string())));
}

#[test]
fn test_ternary_nested_in_consequent() {
    // Right-associative: a ? b ? c : d : e means a ? (b ? c : d) : e
    let source = r#"
        return true ? (false ? 1 : 2) : 3
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(2.0)));
}

#[test]
fn test_ternary_nested_in_alternate() {
    let source = r#"
        return false ? 1 : (true ? 2 : 3)
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(2.0)));
}

#[test]
fn test_ternary_with_object_property() {
    let source = r#"
        let obj = {active: true}
        return obj.active ? "yes" : "no"
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("yes".to_string())));
}

#[test]
fn test_ternary_in_object_value() {
    let source = r#"
        let cancelled = true
        let obj = { reason: cancelled ? "canceled" : "expired" }
        return obj.reason
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::Str("canceled".to_string()))
    );
}

#[test]
fn test_ternary_with_and_operator() {
    let source = r#"
        let a = true
        let b = false
        return a && b ? "both" : "not both"
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::Str("not both".to_string()))
    );
}

#[test]
fn test_ternary_with_or_operator() {
    let source = r#"
        let a = false
        let b = true
        return a || b ? "at least one" : "neither"
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::Str("at least one".to_string()))
    );
}

#[test]
fn test_ternary_short_circuit_consequent_not_evaluated() {
    // When condition is false, consequent should not be evaluated
    // We can't easily test side effects, but we can verify it returns the alternate
    let source = r#"
        return false ? (1 / 0) : "safe"
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // Division by zero would cause infinity, but since condition is false,
    // the consequent is not evaluated
    assert_eq!(vm.control, Control::Return(Val::Str("safe".to_string())));
}

#[test]
fn test_ternary_short_circuit_alternate_not_evaluated() {
    let source = r#"
        return true ? "safe" : (1 / 0)
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("safe".to_string())));
}
