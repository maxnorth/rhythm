//! Tests for error propagation and handling
//!
//! Tests that errors in expressions properly escalate to Control::Throw

use crate::interpreter::executor_v2::errors;
use crate::interpreter::executor_v2::{run_until_done, Control, Stmt, Val, VM};
use std::collections::HashMap;

#[test]
fn test_property_not_found_throws() {
    // Test that accessing a non-existent property throws an error
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Member",
                "object": {
                    "t": "Ident",
                    "name": "obj"
                },
                "property": "missing"
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    let obj = HashMap::new(); // Empty object - no "missing" property
    env.insert("obj".to_string(), Val::Obj(obj));

    let mut vm = VM::new(program, env);
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
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Member",
                "object": {
                    "t": "Ident",
                    "name": "num"
                },
                "property": "foo"
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("num".to_string(), Val::Num(42.0)); // Number, not object

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should throw an error
    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::TYPE_ERROR);
    assert!(err.message.contains("Cannot access property 'foo' on non-object value"));
}

#[test]
fn test_nested_member_access_error_propagates() {
    // Test that errors in nested member access propagate correctly
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Member",
                "object": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "obj"
                    },
                    "property": "inner"
                },
                "property": "missing"
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    let mut obj = HashMap::new();
    // inner exists but is empty
    obj.insert("inner".to_string(), Val::Obj(HashMap::new()));
    env.insert("obj".to_string(), Val::Obj(obj));

    let mut vm = VM::new(program, env);
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
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Member",
                "object": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "obj"
                    },
                    "property": "missing"
                },
                "property": "foo"
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("obj".to_string(), Val::Obj(HashMap::new())); // Empty object

    let mut vm = VM::new(program, env);
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
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Member",
                "object": {
                    "t": "Ident",
                    "name": "obj"
                },
                "property": "missing"
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("obj".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
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
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Await",
                "inner": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "obj"
                    },
                    "property": "missing"
                }
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("obj".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
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
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Block",
            "body": [{
                "t": "Return",
                "value": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "obj"
                    },
                    "property": "missing"
                }
            }]
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("obj".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should have thrown an error
    assert!(matches!(vm.control, Control::Throw(_)));

    // Frames should be cleared
    assert_eq!(vm.frames.len(), 0);
}
