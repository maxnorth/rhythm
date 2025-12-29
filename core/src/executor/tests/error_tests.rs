//! Tests for error propagation and handling
//!
//! Tests that errors in expressions properly escalate to Control::Throw

use super::helpers::parse_workflow_and_build_vm;
use crate::executor::errors;
use crate::executor::{run_until_done, Control, Val, VM};
use std::collections::HashMap;

#[test]
fn test_property_not_found_throws() {
    // Test that accessing a non-existent property throws an error
    let source = r#"
            obj = {}
            return obj.missing
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw an error
    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::PROPERTY_NOT_FOUND);
    assert!(err.message.contains("Property 'missing' not found"));
}

#[test]
fn test_member_access_on_non_object_throws() {
    // Test that accessing a property on a non-object value throws an error
    let source = r#"
            num = 42
            return num.foo
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw an error
    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::TYPE_ERROR);
    assert!(err
        .message
        .contains("Cannot access property 'foo' on non-object value"));
}

#[test]
fn test_nested_member_access_error_propagates() {
    // Test that errors in nested member access propagate correctly
    let source = r#"
            obj = {inner: {}}
            return obj.inner.missing
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw an error for missing property on inner object
    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::PROPERTY_NOT_FOUND);
    assert!(err.message.contains("Property 'missing' not found"));
}

#[test]
fn test_error_in_first_member_access() {
    // Test error in the first step of nested member access
    let source = r#"
            obj = {}
            return obj.missing.foo
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw an error for the first missing property
    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::PROPERTY_NOT_FOUND);
    assert!(err.message.contains("Property 'missing' not found"));
}

#[test]
fn test_error_serialization() {
    // Test that a VM with Control::Throw can be serialized and deserialized
    let source = r#"
            obj = {}
            return obj.missing
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should have thrown an error
    assert!(matches!(vm.control, Control::Throw(_)));

    // Serialize the VM
    let serialized = serde_json::to_string(&vm).unwrap();

    // Deserialize it back
    let vm2: VM = serde_json::from_str(&serialized).unwrap();

    // Should still have the error
    let Control::Throw(Val::Error(err)) = vm2.control else {
        unreachable!(
            "Expected Control::Throw with Error after deserialization, got {:?}",
            vm2.control
        );
    };
    assert_eq!(err.code, errors::PROPERTY_NOT_FOUND);
    assert!(err.message.contains("Property 'missing' not found"));
}

#[test]
fn test_await_propagates_error() {
    // Test that await propagates errors from inner expressions
    let source = r#"
            obj = {}
            return await obj.missing
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw an error (not suspend)
    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::PROPERTY_NOT_FOUND);
    assert!(err.message.contains("Property 'missing' not found"));
}

#[test]
fn test_error_clears_frames() {
    // Test that errors clear the frame stack (like return)
    let source = r#"
            obj = {}
            {
                return obj.missing
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should have thrown an error
    assert!(matches!(vm.control, Control::Throw(_)));

    // Frames should be cleared
    assert_eq!(vm.frames.len(), 0);
}

/* ===================== Try/Catch Tests ===================== */

#[test]
fn test_try_catch_basic() {
    // Test that try/catch catches an error and executes the catch block
    let source = r#"
            obj = {}
            try {
                return obj.missing
            } catch (error) {
                return error
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return the error (not throw it)
    let Control::Return(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Return with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::PROPERTY_NOT_FOUND);
    assert!(err.message.contains("Property 'missing' not found"));
}

#[test]
fn test_try_catch_no_error() {
    // Test that try/catch executes try block when no error occurs
    let source = r#"
            try {
                return 42
            } catch (error) {
                return 999
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return 42 from the try block (not 999 from catch)
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_nested_try_catch() {
    // Test nested try/catch blocks - inner catch should handle error
    let source = r#"
            obj = {}
            try {
                try {
                    return obj.missing
                } catch (inner_error) {
                    return "inner"
                }
            } catch (outer_error) {
                return "outer"
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Inner catch should handle the error
    assert_eq!(vm.control, Control::Return(Val::Str("inner".to_string())));
}

#[test]
fn test_try_catch_propagates_to_outer() {
    // Test that errors in catch block propagate to outer try/catch
    let source = r#"
            obj = {}
            obj2 = {}
            try {
                try {
                    return obj.missing
                } catch (inner_error) {
                    return obj2.also_missing
                }
            } catch (outer_error) {
                return outer_error
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Outer catch should handle the error from the inner catch block
    let Control::Return(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Return with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::PROPERTY_NOT_FOUND);
    assert!(err.message.contains("Property 'also_missing' not found"));
}

#[test]
fn test_try_catch_with_blocks() {
    // Test try/catch with block statements
    let source = r#"
            obj = {}
            try {
                {
                    return obj.missing
                }
            } catch (e) {
                {
                    return "caught"
                }
            }
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return "caught" from the catch block
    assert_eq!(vm.control, Control::Return(Val::Str("caught".to_string())));
}

#[test]
fn test_try_catch_serialization() {
    // Test that try/catch works correctly after serialization/deserialization
    let source = r#"
            obj = {}
            try {
                return obj.missing
            } catch (error) {
                return "serialized"
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Serialize and deserialize
    let serialized = serde_json::to_string(&vm).unwrap();
    let vm2: VM = serde_json::from_str(&serialized).unwrap();

    // Should have the correct result
    assert_eq!(
        vm2.control,
        Control::Return(Val::Str("serialized".to_string()))
    );
}

#[test]
fn test_try_catch_await_error() {
    // Test that errors during await expression evaluation are caught by try/catch
    let source = r#"
            obj = {}
            try {
                return await obj.missing
            } catch (e) {
                return "caught_await_error"
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should catch the error and return the catch block result
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("caught_await_error".to_string()))
    );
}

#[test]
fn test_await_error_uncaught() {
    // Test that errors during await expression evaluation propagate when not caught
    let source = r#"
            obj = {}
            return await obj.missing
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw an error (not caught)
    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::PROPERTY_NOT_FOUND);
    assert!(err.message.contains("Property 'missing' not found"));
}

#[test]
fn test_error_in_catch_handler() {
    // Test that errors thrown inside a catch handler properly propagate
    let source = r#"
            obj = {}
            try {
                {
                    return obj.missing
                }
            } catch (e) {
                {
                    return obj.another_missing
                }
            }
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // The error in the catch handler should propagate to the top level
    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::PROPERTY_NOT_FOUND);
    assert!(err.message.contains("Property 'another_missing' not found"));
}

#[test]
fn test_try_block_variables_not_accessible_in_catch() {
    // Variables declared in try block are NOT accessible in catch block
    // This is standard JavaScript scoping - try and catch are separate block scopes
    let source = r#"
            let obj = {}
            try {
                let email = "test@example.com"
                return obj.missing
            } catch (e) {
                return email
            }
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should fail with undefined variable error - email is out of scope in catch
    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!(
            "Expected Control::Throw with undefined variable error, got {:?}",
            vm.control
        );
    };
    assert!(
        err.message.contains("Undefined variable"),
        "Expected undefined variable error, got: {}",
        err.message
    );
}

#[test]
fn test_try_catch_variable_declared_outside() {
    // Variables declared OUTSIDE try block ARE accessible in catch block
    let source = r#"
            let obj = {}
            let email = "test@example.com"
            try {
                return obj.missing
            } catch (e) {
                return email
            }
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return the email since it's in scope
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("test@example.com".to_string()))
    );
}

#[test]
fn test_try_block_completes_without_infinite_loop() {
    // Regression test: try block should complete normally without restarting
    // Previously, the try frame stayed in ExecuteTry phase after body completed,
    // causing the body to be pushed again in an infinite loop
    let source = r#"
            let count = 0
            try {
                count = count + 1
            } catch (e) {
                count = 999
            }
            return count
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return 1, not 999 (no error occurred)
    assert_eq!(vm.control, Control::Return(Val::Num(1.0)));
}

#[test]
fn test_try_with_multiple_statements_completes() {
    // Verify try block with multiple statements completes normally
    let source = r#"
            let a = 0
            let b = 0
            try {
                a = 1
                b = 2
            } catch (e) {
                a = 999
            }
            return a + b
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return 3 (1 + 2)
    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}
