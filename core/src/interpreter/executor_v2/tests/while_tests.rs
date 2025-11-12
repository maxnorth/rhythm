//! Tests for While statements

use super::super::*;
use super::helpers::parse_workflow_and_build_vm;
use maplit::hashmap;
use std::collections::HashMap;

#[test]
fn test_while_simple_loop() {
    // i = 0; while (i < 3) { i = i + 1; } return i;
    let source = r#"
            i = 0
            while (lt(i, 3)) {
                i = add(i, 1)
            }
            return i
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
    assert_eq!(vm.env.get("i"), Some(&Val::Num(3.0)));
}

#[test]
fn test_while_zero_iterations() {
    // while (false) { return 1; } return 2;
    let source = r#"
            while (false) {
                return 1
            }
            return 2
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(2.0)));
}

#[test]
fn test_while_with_break() {
    // i = 0; while (true) { if (i >= 5) { break; } i = i + 1; } return i;
    let source = r#"
            i = 0
            while (true) {
                if (gte(i, 5)) {
                    break
                }
                i = add(i, 1)
            }
            return i
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(5.0)));
}

#[test]
fn test_while_with_continue() {
    // i = 0; sum = 0; while (i < 5) { i = i + 1; if (i == 3) { continue; } sum = sum + i; } return sum;
    let source = r#"
            i = 0
            sum = 0
            while (lt(i, 5)) {
                i = add(i, 1)
                if (eq(i, 3)) {
                    continue
                }
                sum = add(sum, i)
            }
            return sum
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // sum = 1 + 2 + 4 + 5 = 12 (skipping 3)
    assert_eq!(vm.control, Control::Return(Val::Num(12.0)));
}

#[test]
fn test_while_nested() {
    // i = 0; j = 0; while (i < 2) { while (j < 2) { j = j + 1; } i = i + 1; j = 0; } return i;
    let source = r#"
            i = 0
            j = 0
            while (lt(i, 2)) {
                while (lt(j, 2)) {
                    j = add(j, 1)
                }
                i = add(i, 1)
                j = 0
            }
            return i
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(2.0)));
}

#[test]
fn test_while_with_return() {
    // i = 0; while (i < 10) { i = i + 1; if (i == 5) { return i; } } return 99;
    let source = r#"
            i = 0
            while (lt(i, 10)) {
                i = add(i, 1)
                if (eq(i, 5)) {
                    return i
                }
            }
            return 99
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(5.0)));
}

#[test]
fn test_while_with_error_in_test() {
    // while (Context.bad) { return 1; }
    // ctx doesn't have 'bad' property, so this should throw
    let source = r#"
            while (Context.bad) {
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
fn test_while_with_try_catch() {
    // i = 0; while (i < 5) { try { if (i == 3) { throw {code: "E", message: "msg"}; } i = i + 1; } catch (e) { i = 10; } } return i;
    let source = r#"
            i = 0
            while (lt(i, 5)) {
                try {
                    if (eq(i, 3)) {
                        throw({code: "E", message: "msg"})
                    }
                    i = add(i, 1)
                } catch (e) {
                    i = 10
                }
            }
            return i
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Loop runs 0,1,2 iterations normally, then throws on i=3, catch sets i=10, loop exits
    assert_eq!(vm.control, Control::Return(Val::Num(10.0)));
}

#[test]
fn test_while_accumulator() {
    // sum = 0; i = 1; while (i <= 5) { sum = sum + i; i = i + 1; } return sum;
    let source = r#"
            sum = 0
            i = 1
            while (lte(i, 5)) {
                sum = add(sum, i)
                i = add(i, 1)
            }
            return sum
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // sum = 1 + 2 + 3 + 4 + 5 = 15
    assert_eq!(vm.control, Control::Return(Val::Num(15.0)));
}

/* ===================== Parser-based While Tests ===================== */

#[test]
fn test_while_false_exits_immediately() {
    let source = r#"
            while (false) {
                return 99
            }
            return 42
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_while_break_exits() {
    let source = r#"
            while (true) {
                break
            }
            return 100
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(100.0)));
}

#[test]
fn test_while_break_in_nested_block() {
    let source = r#"
            while (true) {
                {
                    break
                }
                return 99
            }
            return 42
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_while_return_exits_immediately() {
    let source = r#"
            while (true) {
                return 55
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(55.0)));
}

#[test]
fn test_nested_while_with_breaks() {
    let source = r#"
            while (true) {
                while (true) {
                    break
                }
                break
            }
            return 77
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(77.0)));
}
