//! Tests for JavaScript-style truthiness

use super::helpers::parse_workflow_and_build_vm;
use crate::executor::{run_until_done, Control, Val};
use std::collections::HashMap;

/* ===================== Truthiness in if statements ===================== */

#[test]
fn test_if_with_number_truthy() {
    // if (5) should be truthy
    let source = r#"
            if (5) {
                return "truthy"
            }
            return "falsy"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("truthy".to_string())));
}

#[test]
fn test_if_with_zero_falsy() {
    // if (0) should be falsy
    let source = r#"
        if (0) {
            return "truthy"
        }
        return "falsy"
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("falsy".to_string())));
}

#[test]
fn test_if_with_empty_string_falsy() {
    // if ("") should be falsy
    let source = r#"
            if ("") {
                return "truthy"
            }
            return "falsy"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("falsy".to_string())));
}

#[test]
fn test_if_with_non_empty_string_truthy() {
    // if ("hello") should be truthy
    let source = r#"
            if ("hello") {
                return "truthy"
            }
            return "falsy"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("truthy".to_string())));
}

#[test]
fn test_if_with_string_zero_truthy() {
    // if ("0") should be truthy (string "0", not number 0)
    let source = r#"
            if ("0") {
                return "truthy"
            }
            return "falsy"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("truthy".to_string())));
}

#[test]
fn test_if_with_null_falsy() {
    // if (null) should be falsy
    let source = r#"
            if (null) {
                return "truthy"
            }
            return "falsy"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("falsy".to_string())));
}

#[test]
fn test_if_with_empty_array_truthy() {
    // if ([]) should be truthy (even though array is empty)
    let source = r#"
            if ([]) {
                return "truthy"
            }
            return "falsy"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("truthy".to_string())));
}

#[test]
fn test_if_with_empty_object_truthy() {
    // if ({}) should be truthy (even though object is empty)
    let source = r#"
            if ({}) {
                return "truthy"
            }
            return "falsy"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("truthy".to_string())));
}

/* ===================== Truthiness in while statements ===================== */

#[test]
fn test_while_with_number() {
    // while (n) with n starting as 3, decrementing to 0
    let source = r#"
            n = 3
            count = 0
            while (n) {
                count = count + 1
                n = n - 1
            }
            return count
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}

/* ===================== Truthiness in logical operators ===================== */

#[test]
fn test_and_with_truthy_number() {
    // 5 && 10 should return 10 (right value when left is truthy)
    let source = r#"
            return 5 && 10
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(10.0)));
}

#[test]
fn test_and_with_zero() {
    // 0 && true should return 0 (left value is falsy)
    let source = r#"
            return 0 && true
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(0.0)));
}

#[test]
fn test_and_with_empty_string() {
    // "" && true should return "" (left value is falsy)
    let source = r#"
            return "" && true
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("".to_string())));
}

#[test]
fn test_or_with_zero() {
    // 0 || true should evaluate to true
    let source = r#"
            return 0 || true
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_or_with_empty_string() {
    // "" || false should evaluate to false (both are falsy)
    let source = r#"
            return "" || false
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_or_with_non_empty_string() {
    // "hello" || false should return "hello" (left value is truthy)
    let source = r#"
            return "hello" || false
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("hello".to_string())));
}

#[test]
fn test_and_with_null() {
    // null && true should return null (left value is falsy)
    let source = r#"
            return null && true
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Null));
}

#[test]
fn test_or_with_null() {
    // null || "default" should return "default" (right value when left is falsy)
    let source = r#"
            return null || "default"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("default".to_string())));
}

#[test]
fn test_and_with_array() {
    // [1, 2] && true should evaluate to true (arrays are truthy)
    let source = r#"
            return [1, 2] && true
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_and_with_object() {
    // {a: 1} && false should evaluate to false
    let source = r#"
            return {a: 1} && false
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

/* ===================== Complex truthiness scenarios ===================== */

#[test]
fn test_truthiness_in_complex_expression() {
    // (0 || 5) && ("" || "hello")
    // = 5 && "hello" (OR returns the truthy right values)
    // = "hello" (AND returns right value when left is truthy)
    let source = r#"
            return (0 || 5) && ("" || "hello")
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("hello".to_string())));
}

#[test]
fn test_truthiness_with_comparison() {
    // x = 0; if (x || x == 0) should be true
    let source = r#"
            x = 0
            if (x || x == 0) {
                return "truthy"
            }
            return "falsy"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("truthy".to_string())));
}

/* ===================== NOT Operator Tests ===================== */

#[test]
fn test_not_true() {
    // !true should be false
    let source = r#"
            return !true
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_not_false() {
    // !false should be true
    let source = r#"
            return !false
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_not_truthy_number() {
    // !5 should be false (5 is truthy)
    let source = r#"
            return !5
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_not_zero() {
    // !0 should be true (0 is falsy)
    let source = r#"
            return !0
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_not_empty_string() {
    // !"" should be true (empty string is falsy)
    let source = r#"
            return !""
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_not_non_empty_string() {
    // !"hello" should be false (non-empty string is truthy)
    let source = r#"
            return !"hello"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_not_null() {
    // !null should be true (null is falsy)
    let source = r#"
            return !null
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_double_not() {
    // !!5 should be true (converts to boolean)
    let source = r#"
            return !!5
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_double_not_zero() {
    // !!0 should be false
    let source = r#"
            return !!0
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_not_in_if() {
    // if (!false) should execute then branch
    let source = r#"
            if (!false) {
                return "executed"
            }
            return "not executed"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("executed".to_string()))
    );
}

#[test]
fn test_not_with_comparison() {
    // !(5 > 10) should be true
    let source = r#"
            return !(5 > 10)
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_not_with_and() {
    // !(true && false) should be true
    let source = r#"
            return !(true && false)
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_not_with_or() {
    // !(false || false) should be true
    let source = r#"
            return !(false || false)
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_de_morgan_law() {
    // !(true && false) == (!true || !false)
    // = !(false) == (false || true)
    // = true == true
    let source = r#"
            a = !(true && false)
            b = !true || !false
            return a == b
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}
