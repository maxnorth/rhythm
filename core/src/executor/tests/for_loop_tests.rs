//! Tests for ForLoop statements (for...in / for...of)

use super::super::*;
use super::helpers::parse_workflow_and_build_vm;
use maplit::hashmap;

/* ===================== for...of Tests ===================== */

#[test]
fn test_for_of_simple() {
    let source = r#"
        let arr = [1, 2, 3]
        let sum = 0
        for (let x of arr) {
            sum = sum + x
        }
        return sum
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(6.0)));
}

#[test]
fn test_for_of_empty_array() {
    let source = r#"
        let arr = []
        let count = 0
        for (let x of arr) {
            count = count + 1
        }
        return count
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(0.0)));
}

#[test]
fn test_for_of_with_break() {
    let source = r#"
        let arr = [1, 2, 3, 4, 5]
        let sum = 0
        for (let x of arr) {
            if (x == 4) {
                break
            }
            sum = sum + x
        }
        return sum
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // sum = 1 + 2 + 3 = 6
    assert_eq!(vm.control, Control::Return(Val::Num(6.0)));
}

#[test]
fn test_for_of_with_continue() {
    let source = r#"
        let arr = [1, 2, 3, 4, 5]
        let sum = 0
        for (let x of arr) {
            if (x == 3) {
                continue
            }
            sum = sum + x
        }
        return sum
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // sum = 1 + 2 + 4 + 5 = 12
    assert_eq!(vm.control, Control::Return(Val::Num(12.0)));
}

#[test]
fn test_for_of_with_return() {
    let source = r#"
        let arr = [1, 2, 3, 4, 5]
        for (let x of arr) {
            if (x == 3) {
                return x
            }
        }
        return 0
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}

#[test]
fn test_for_of_nested() {
    let source = r#"
        let arr1 = [1, 2]
        let arr2 = [10, 20]
        let sum = 0
        for (let a of arr1) {
            for (let b of arr2) {
                sum = sum + a + b
            }
        }
        return sum
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // (1+10) + (1+20) + (2+10) + (2+20) = 11 + 21 + 12 + 22 = 66
    assert_eq!(vm.control, Control::Return(Val::Num(66.0)));
}

#[test]
fn test_for_of_with_objects() {
    let source = r#"
        let items = [{name: "a", value: 1}, {name: "b", value: 2}]
        let sum = 0
        for (let item of items) {
            sum = sum + item.value
        }
        return sum
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}

#[test]
fn test_for_of_non_array_throws() {
    let source = r#"
        let obj = {a: 1}
        for (let x of obj) {
            return x
        }
        return 0
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    match vm.control {
        Control::Throw(Val::Error(ref err)) => {
            assert_eq!(err.code, "TYPE_ERROR");
            assert!(err.message.contains("array"));
        }
        _ => panic!("Expected TYPE_ERROR, got {:?}", vm.control),
    }
}

/* ===================== for...in Tests ===================== */

#[test]
fn test_for_in_object_keys() {
    let source = r#"
        let obj = {a: 1, b: 2, c: 3}
        let count = 0
        for (let k in obj) {
            count = count + 1
        }
        return count
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // Should have 3 keys
    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}

#[test]
fn test_for_in_array_indices() {
    let source = r#"
        let arr = ["a", "b", "c"]
        let sum = 0
        for (let i in arr) {
            sum = sum + i
        }
        return sum
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // sum = 0 + 1 + 2 = 3
    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}

#[test]
fn test_for_in_with_break() {
    let source = r#"
        let obj = {a: 1, b: 2, c: 3}
        let count = 0
        for (let k in obj) {
            count = count + 1
            if (count == 2) {
                break
            }
        }
        return count
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(2.0)));
}

#[test]
fn test_for_in_empty_object() {
    let source = r#"
        let obj = {}
        let count = 0
        for (let k in obj) {
            count = count + 1
        }
        return count
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(0.0)));
}

#[test]
fn test_for_in_non_object_throws() {
    let source = r#"
        let x = 42
        for (let k in x) {
            return k
        }
        return 0
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    match vm.control {
        Control::Throw(Val::Error(ref err)) => {
            assert_eq!(err.code, "TYPE_ERROR");
            assert!(err.message.contains("object or array"));
        }
        _ => panic!("Expected TYPE_ERROR, got {:?}", vm.control),
    }
}

/* ===================== Scope Cleanup Tests ===================== */

#[test]
fn test_for_of_binding_not_in_scope_after_loop() {
    let source = r#"
        let arr = [1, 2, 3]
        for (let x of arr) {
            let y = x
        }
        return x
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // x should not be in scope after the loop
    match vm.control {
        Control::Throw(Val::Error(ref err)) => {
            assert!(err.message.contains("Undefined variable"));
        }
        _ => panic!(
            "Expected error for undefined variable, got {:?}",
            vm.control
        ),
    }
}

#[test]
fn test_for_of_with_const() {
    let source = r#"
        let arr = [1, 2, 3]
        let sum = 0
        for (const x of arr) {
            sum = sum + x
        }
        return sum
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(6.0)));
}
