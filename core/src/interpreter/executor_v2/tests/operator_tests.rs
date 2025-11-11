//! Tests for binary operators (&&, ||, ==, !=, <, <=, >, >=, +, -, *, /)

use super::helpers::parse_workflow_and_build_vm;
use crate::interpreter::executor_v2::{run_until_done, Control, Val};
use std::collections::HashMap;

/* ===================== Arithmetic Operators ===================== */

#[test]
fn test_add_basic() {
    let source = r#"
        async function workflow() {
            return 1 + 2
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}

#[test]
fn test_sub_basic() {
    let source = r#"
        async function workflow() {
            return 5 - 3
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(2.0)));
}

#[test]
fn test_mul_basic() {
    let source = r#"
        async function workflow() {
            return 3 * 4
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(12.0)));
}

#[test]
fn test_div_basic() {
    let source = r#"
        async function workflow() {
            return 10 / 2
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(5.0)));
}

#[test]
fn test_arithmetic_precedence() {
    // 2 + 3 * 4 should be 2 + (3 * 4) = 14, not (2 + 3) * 4 = 20
    let source = r#"
        async function workflow() {
            return 2 + 3 * 4
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(14.0)));
}

#[test]
fn test_arithmetic_complex() {
    // (10 + 5) * 2 - 3 = 15 * 2 - 3 = 30 - 3 = 27
    let source = r#"
        async function workflow() {
            x = 10 + 5
            y = x * 2
            return y - 3
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(27.0)));
}

/* ===================== Comparison Operators ===================== */

#[test]
fn test_eq_numbers_true() {
    let source = r#"
        async function workflow() {
            return 5 == 5
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_eq_numbers_false() {
    let source = r#"
        async function workflow() {
            return 5 == 3
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_ne_numbers_true() {
    let source = r#"
        async function workflow() {
            return 5 != 3
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_ne_numbers_false() {
    let source = r#"
        async function workflow() {
            return 5 != 5
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_lt_true() {
    let source = r#"
        async function workflow() {
            return 3 < 5
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_lt_false() {
    let source = r#"
        async function workflow() {
            return 5 < 3
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_lte_true_less() {
    let source = r#"
        async function workflow() {
            return 3 <= 5
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_lte_true_equal() {
    let source = r#"
        async function workflow() {
            return 5 <= 5
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_gt_true() {
    let source = r#"
        async function workflow() {
            return 5 > 3
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_gte_true_greater() {
    let source = r#"
        async function workflow() {
            return 5 >= 3
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_gte_true_equal() {
    let source = r#"
        async function workflow() {
            return 5 >= 5
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

/* ===================== Logical Operators ===================== */

#[test]
fn test_and_true_true() {
    let source = r#"
        async function workflow() {
            return true && true
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_and_true_false() {
    let source = r#"
        async function workflow() {
            return true && false
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_and_false_true() {
    let source = r#"
        async function workflow() {
            return false && true
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_and_false_false() {
    let source = r#"
        async function workflow() {
            return false && false
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_or_true_true() {
    let source = r#"
        async function workflow() {
            return true || true
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_or_true_false() {
    let source = r#"
        async function workflow() {
            return true || false
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_or_false_true() {
    let source = r#"
        async function workflow() {
            return false || true
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_or_false_false() {
    let source = r#"
        async function workflow() {
            return false || false
        }
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
        async function workflow() {
            return 5 + 3 > 7
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_logical_with_comparison() {
    // 5 > 3 && 10 < 20 should be true && true = true
    let source = r#"
        async function workflow() {
            return 5 > 3 && 10 < 20
        }
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
        async function workflow() {
            return true || false && false
        }
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
        async function workflow() {
            x = 10
            y = 5
            return x > 5 && y < 10 || x == 0
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_operators_in_if_condition() {
    let source = r#"
        async function workflow() {
            x = 10
            if (x > 5 && x < 15) {
                return 1
            }
            return 0
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(1.0)));
}

#[test]
fn test_operators_in_while_condition() {
    let source = r#"
        async function workflow() {
            x = 0
            while (x < 3) {
                x = x + 1
            }
            return x
        }
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
        async function workflow() {
            return (2 + 3) * 4
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(20.0)));
}

#[test]
fn test_nested_parentheses() {
    // ((2 + 3) * 4) + 1 = (5 * 4) + 1 = 20 + 1 = 21
    let source = r#"
        async function workflow() {
            return ((2 + 3) * 4) + 1
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(21.0)));
}

#[test]
fn test_parentheses_with_comparison() {
    // (5 + 3) > 7 should be 8 > 7 = true
    let source = r#"
        async function workflow() {
            return (5 + 3) > 7
        }
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
        async function workflow() {
            return (true || false) && false
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_multiple_parentheses_groups() {
    // (2 + 3) * (4 + 1) = 5 * 5 = 25
    let source = r#"
        async function workflow() {
            return (2 + 3) * (4 + 1)
        }
    "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(25.0)));
}
