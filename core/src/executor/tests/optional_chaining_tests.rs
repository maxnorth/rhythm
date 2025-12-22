//! Tests for optional chaining (?.) operator

use super::helpers::parse_workflow_and_build_vm;
use crate::executor::{run_until_done, Control, Val};
use std::collections::HashMap;

/* ===================== Basic Optional Chaining ===================== */

#[test]
fn test_optional_chaining_with_null() {
    // obj?.prop where obj is null should return null
    let source = r#"
            obj = null
            return obj?.prop
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Null));
}

#[test]
fn test_optional_chaining_with_object() {
    // obj?.prop where obj exists should return property value
    let source = r#"
            obj = {prop: "value"}
            return obj?.prop
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("value".to_string())));
}

#[test]
fn test_regular_access_with_null_throws() {
    // obj.prop where obj is null should throw an error
    let source = r#"
            obj = null
            return obj.prop
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should result in an error
    match vm.control {
        Control::Throw(_) => {
            // Expected - null property access should throw
        }
        _ => panic!("Expected error when accessing property on null without optional chaining"),
    }
}

#[test]
fn test_optional_chaining_deep_property() {
    // obj?.nested where obj has nested property
    let source = r#"
            obj = {nested: {value: 42}}
            return obj?.nested
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    match vm.control {
        Control::Return(Val::Obj(map)) => {
            assert_eq!(map.get("value"), Some(&Val::Num(42.0)));
        }
        _ => panic!("Expected object with nested value"),
    }
}

/* ===================== Chained Optional Access ===================== */

#[test]
fn test_chained_optional_access_all_present() {
    // obj?.a?.b where all properties exist
    let source = r#"
            obj = {a: {b: "success"}}
            return obj?.a?.b
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("success".to_string())));
}

#[test]
fn test_chained_optional_access_first_null() {
    // obj?.a?.b where obj is null
    let source = r#"
            obj = null
            return obj?.a?.b
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Null));
}

#[test]
fn test_chained_optional_access_middle_null() {
    // obj?.a?.b where obj.a is null
    let source = r#"
            obj = {a: null}
            return obj?.a?.b
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Null));
}

/* ===================== Mixed Regular and Optional Access ===================== */

#[test]
fn test_mixed_regular_then_optional() {
    // obj.a?.b where obj and obj.a exist
    let source = r#"
            obj = {a: {b: "mixed"}}
            return obj.a?.b
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("mixed".to_string())));
}

#[test]
fn test_mixed_optional_then_regular() {
    // obj?.a.b where obj and obj.a and obj.a.b exist
    let source = r#"
            obj = {a: {b: "mixed2"}}
            return obj?.a.b
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("mixed2".to_string())));
}

#[test]
fn test_mixed_optional_then_regular_with_null() {
    // obj?.a.b where obj is null
    // First obj?.a returns null, then null.b throws
    let source = r#"
            obj = null
            return obj?.a.b
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw because null.b is a type error
    match vm.control {
        Control::Throw(_) => {
            // Expected - accessing property on null with regular access throws
        }
        _ => panic!("Expected error when accessing property on null with regular access"),
    }
}

/* ===================== Optional Chaining in Expressions ===================== */

#[test]
fn test_optional_chaining_with_default_value() {
    // (obj?.prop || "default") pattern
    let source = r#"
            obj = null
            return obj?.prop || "default"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("default".to_string())));
}

#[test]
fn test_optional_chaining_in_if_condition() {
    // if (obj?.prop) with null obj
    let source = r#"
            obj = null
            if (obj?.prop) {
                return "truthy"
            }
            return "falsy"
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("falsy".to_string())));
}

#[test]
fn test_optional_chaining_in_assignment() {
    // x = obj?.prop
    let source = r#"
            obj = null
            x = obj?.prop
            return x
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Null));
}

/* ===================== Edge Cases ===================== */

#[test]
fn test_optional_chaining_with_numeric_value() {
    // obj?.prop where obj is a number (not an object)
    let source = r#"
            obj = 42
            return obj?.prop
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw because 42 is not an object and not null
    match vm.control {
        Control::Throw(_) => {
            // Expected - can't access properties on numbers even with ?.
        }
        _ => panic!("Expected error when accessing property on number"),
    }
}

#[test]
fn test_optional_chaining_with_string_value() {
    // obj?.prop where obj is a string
    let source = r#"
            obj = "hello"
            return obj?.prop
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw because strings don't have properties (in our implementation)
    match vm.control {
        Control::Throw(_) => {
            // Expected
        }
        _ => panic!("Expected error when accessing property on string"),
    }
}

#[test]
fn test_triple_optional_chaining() {
    // obj?.a?.b?.c with all nulls at various levels
    let source = r#"
            obj = {a: {b: null}}
            return obj?.a?.b?.c
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Null));
}

#[test]
fn test_optional_chaining_returns_number() {
    // Ensure optional chaining preserves type
    let source = r#"
            obj = {count: 0}
            return obj?.count
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(0.0)));
}

#[test]
fn test_optional_chaining_with_boolean() {
    // Ensure optional chaining preserves boolean values
    let source = r#"
            obj = {flag: false}
            return obj?.flag
        "#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}
