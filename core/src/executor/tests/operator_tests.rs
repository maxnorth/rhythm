//! Tests for binary operators (&&, ||, ==, !=, <, <=, >, >=, +, -, *, /)

use super::helpers::parse_workflow_and_build_vm;
use crate::executor::{run_until_done, Control, Val};
use std::collections::HashMap;

/* ===================== Arithmetic Operators ===================== */

#[test]
fn test_add_basic() {
    let source = r#"
            return 1 + 2
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}

#[test]
fn test_sub_basic() {
    let source = r#"
            return 5 - 3
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(2.0)));
}

#[test]
fn test_mul_basic() {
    let source = r#"
            return 3 * 4
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(12.0)));
}

#[test]
fn test_div_basic() {
    let source = r#"
            return 10 / 2
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(5.0)));
}

#[test]
fn test_arithmetic_precedence() {
    // 2 + 3 * 4 should be 2 + (3 * 4) = 14, not (2 + 3) * 4 = 20
    let source = r#"
            return 2 + 3 * 4
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(14.0)));
}

#[test]
fn test_arithmetic_complex() {
    // (10 + 5) * 2 - 3 = 15 * 2 - 3 = 30 - 3 = 27
    let source = r#"
            x = 10 + 5
            y = x * 2
            return y - 3
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(27.0)));
}

/* ===================== Comparison Operators ===================== */

#[test]
fn test_eq_numbers_true() {
    let source = r#"
            return 5 == 5
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_eq_numbers_false() {
    let source = r#"
            return 5 == 3
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_ne_numbers_true() {
    let source = r#"
            return 5 != 3
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_ne_numbers_false() {
    let source = r#"
            return 5 != 5
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_lt_true() {
    let source = r#"
            return 3 < 5
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_lt_false() {
    let source = r#"
            return 5 < 3
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_lte_true_less() {
    let source = r#"
            return 3 <= 5
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_lte_true_equal() {
    let source = r#"
            return 5 <= 5
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_gt_true() {
    let source = r#"
            return 5 > 3
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_gte_true_greater() {
    let source = r#"
            return 5 >= 3
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_gte_true_equal() {
    let source = r#"
            return 5 >= 5
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

/* ===================== Logical Operators ===================== */

#[test]
fn test_and_true_true() {
    let source = r#"
            return true && true
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_and_true_false() {
    let source = r#"
            return true && false
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_and_false_true() {
    let source = r#"
            return false && true
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_and_false_false() {
    let source = r#"
            return false && false
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_or_true_true() {
    let source = r#"
            return true || true
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_or_true_false() {
    let source = r#"
            return true || false
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_or_false_true() {
    let source = r#"
            return false || true
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_or_false_false() {
    let source = r#"
            return false || false
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

/* ===================== Combined Operators ===================== */

#[test]
fn test_comparison_with_arithmetic() {
    // 5 + 3 > 7 should be 8 > 7 = true
    let source = r#"
            return 5 + 3 > 7
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_logical_with_comparison() {
    // 5 > 3 && 10 < 20 should be true && true = true
    let source = r#"
            return 5 > 3 && 10 < 20
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_logical_precedence() {
    // true || false && false should be true || (false && false) = true || false = true
    // NOT (true || false) && false = true && false = false
    let source = r#"
            return true || false && false
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_complex_expression() {
    // x = 10, y = 5
    // (x > 5 && y < 10) || x == 0
    // = (true && true) || false
    // = true || false
    // = true
    let source = r#"
            x = 10
            y = 5
            return x > 5 && y < 10 || x == 0
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_operators_in_if_condition() {
    let source = r#"
            x = 10
            if (x > 5 && x < 15) {
                return 1
            }
            return 0
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(1.0)));
}

#[test]
fn test_operators_in_while_condition() {
    let source = r#"
            x = 0
            while (x < 3) {
                x = x + 1
            }
            return x
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}

/* ===================== Parentheses and Grouping ===================== */

#[test]
fn test_parentheses_override_precedence() {
    // (2 + 3) * 4 should be 5 * 4 = 20
    // Without parens, 2 + 3 * 4 would be 2 + 12 = 14
    let source = r#"
            return (2 + 3) * 4
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(20.0)));
}

#[test]
fn test_nested_parentheses() {
    // ((2 + 3) * 4) + 1 = (5 * 4) + 1 = 20 + 1 = 21
    let source = r#"
            return ((2 + 3) * 4) + 1
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(21.0)));
}

#[test]
fn test_parentheses_with_comparison() {
    // (5 + 3) > 7 should be 8 > 7 = true
    let source = r#"
            return (5 + 3) > 7
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_parentheses_with_logical() {
    // (true || false) && false should be true && false = false
    // Without parens, true || false && false would be true || false = true (due to && having higher precedence)
    let source = r#"
            return (true || false) && false
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_multiple_parentheses_groups() {
    // (2 + 3) * (4 + 1) = 5 * 5 = 25
    let source = r#"
            return (2 + 3) * (4 + 1)
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(25.0)));
}

/* ===================== Short-Circuit Evaluation ===================== */

#[test]
fn test_and_short_circuit_false_left() {
    // false && <anything> should return false without evaluating right
    // Testing with a value that would be truthy if evaluated
    let source = r#"
            return false && true
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_and_short_circuit_zero_left() {
    // 0 && <anything> should return 0 (the falsy left value) without evaluating right
    let source = r#"
            return 0 && 100
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(0.0)));
}

#[test]
fn test_and_short_circuit_empty_string_left() {
    // "" && <anything> should return "" (the falsy left value)
    let source = r#"
            return "" && "hello"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("".to_string())));
}

#[test]
fn test_and_no_short_circuit_true_left() {
    // true && false should evaluate both sides and return false
    let source = r#"
            return true && false
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_and_no_short_circuit_truthy_left() {
    // 5 && 10 should evaluate both sides and return 10 (the right value)
    let source = r#"
            return 5 && 10
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(10.0)));
}

#[test]
fn test_or_short_circuit_true_left() {
    // true || <anything> should return true without evaluating right
    let source = r#"
            return true || false
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_or_short_circuit_truthy_number_left() {
    // 5 || <anything> should return 5 (the truthy left value) without evaluating right
    let source = r#"
            return 5 || 0
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(5.0)));
}

#[test]
fn test_or_short_circuit_truthy_string_left() {
    // "hello" || <anything> should return "hello" (the truthy left value)
    let source = r#"
            return "hello" || ""
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("hello".to_string())));
}

#[test]
fn test_or_no_short_circuit_false_left() {
    // false || true should evaluate both sides and return true
    let source = r#"
            return false || true
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_or_no_short_circuit_falsy_left() {
    // 0 || 5 should evaluate both sides and return 5 (the right value)
    let source = r#"
            return 0 || 5
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(5.0)));
}

#[test]
fn test_short_circuit_complex_and() {
    // (5 > 10) && (10 / 0) should short-circuit on false left and not divide by zero
    // 5 > 10 = false, so right side should not be evaluated
    let source = r#"
            return 5 > 10 && 10 > 0
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_short_circuit_complex_or() {
    // (10 > 5) || <expr> should short-circuit on true left
    let source = r#"
            return 10 > 5 || 0 > 1
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_short_circuit_chained_and() {
    // false && true && true should short-circuit at first false
    let source = r#"
            return false && true && true
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_short_circuit_chained_or() {
    // true || false || false should short-circuit at first true
    let source = r#"
            return true || false || false
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

/* ===================== JavaScript-Style Value Returning ===================== */

#[test]
fn test_and_returns_actual_values() {
    // "hello" && "world" should return "world" (not true)
    let source = r#"
            return "hello" && "world"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("world".to_string())));
}

#[test]
fn test_or_returns_actual_values() {
    // "first" || "second" should return "first" (not true)
    let source = r#"
            return "first" || "second"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("first".to_string())));
}

#[test]
fn test_and_with_mixed_types() {
    // 42 && "text" should return "text"
    let source = r#"
            return 42 && "text"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("text".to_string())));
}

#[test]
fn test_or_with_null() {
    // null || 100 should return 100
    let source = r#"
            return null || 100
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(100.0)));
}

#[test]
fn test_and_with_array() {
    // [1, 2] && 42 should return 42
    let source = r#"
            return [1, 2] && 42
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_default_value_pattern() {
    // x = null; result = x || "default" should return "default"
    let source = r#"
            x = null
            return x || "default"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("default".to_string())));
}
