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

/* ===================== Try/Catch Tests ===================== */

#[test]
fn test_try_catch_basic() {
    // Test that try/catch catches an error and executes the catch block
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Try",
            "body": {
                "t": "Return",
                "value": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "obj"
                    },
                    "property": "missing"
                }
            },
            "catch_var": "error",
            "catch_body": {
                "t": "Return",
                "value": {
                    "t": "Ident",
                    "name": "error"
                }
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("obj".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should return the error (not throw it)
    let Control::Return(Val::Error(err)) = vm.control else {
        unreachable!(
            "Expected Control::Return with Error, got {:?}",
            vm.control
        );
    };
    assert_eq!(err.code, errors::PROPERTY_NOT_FOUND);
    assert!(err.message.contains("Property 'missing' not found"));
}

#[test]
fn test_try_catch_no_error() {
    // Test that try/catch executes try block when no error occurs
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Try",
            "body": {
                "t": "Return",
                "value": {
                    "t": "LitNum",
                    "v": 42.0
                }
            },
            "catch_var": "error",
            "catch_body": {
                "t": "Return",
                "value": {
                    "t": "LitNum",
                    "v": 999.0
                }
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    // Should return 42 from the try block (not 999 from catch)
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_nested_try_catch() {
    // Test nested try/catch blocks - inner catch should handle error
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Try",
            "body": {
                "t": "Try",
                "body": {
                    "t": "Return",
                    "value": {
                        "t": "Member",
                        "object": {
                            "t": "Ident",
                            "name": "obj"
                        },
                        "property": "missing"
                    }
                },
                "catch_var": "inner_error",
                "catch_body": {
                    "t": "Return",
                    "value": {
                        "t": "LitStr",
                        "v": "inner"
                    }
                }
            },
            "catch_var": "outer_error",
            "catch_body": {
                "t": "Return",
                "value": {
                    "t": "LitStr",
                    "v": "outer"
                }
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("obj".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Inner catch should handle the error
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("inner".to_string()))
    );
}

#[test]
fn test_try_catch_propagates_to_outer() {
    // Test that errors in catch block propagate to outer try/catch
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Try",
            "body": {
                "t": "Try",
                "body": {
                    "t": "Return",
                    "value": {
                        "t": "Member",
                        "object": {
                            "t": "Ident",
                            "name": "obj"
                        },
                        "property": "missing"
                    }
                },
                "catch_var": "inner_error",
                "catch_body": {
                    "t": "Return",
                    "value": {
                        "t": "Member",
                        "object": {
                            "t": "Ident",
                            "name": "obj2"
                        },
                        "property": "also_missing"
                    }
                }
            },
            "catch_var": "outer_error",
            "catch_body": {
                "t": "Return",
                "value": {
                    "t": "Ident",
                    "name": "outer_error"
                }
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("obj".to_string(), Val::Obj(HashMap::new()));
    env.insert("obj2".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Outer catch should handle the error from the inner catch block
    let Control::Return(Val::Error(err)) = vm.control else {
        unreachable!(
            "Expected Control::Return with Error, got {:?}",
            vm.control
        );
    };
    assert_eq!(err.code, errors::PROPERTY_NOT_FOUND);
    assert!(err.message.contains("Property 'also_missing' not found"));
}

#[test]
fn test_try_catch_with_blocks() {
    // Test try/catch with block statements
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Try",
            "body": {
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
            },
            "catch_var": "e",
            "catch_body": {
                "t": "Block",
                "body": [{
                    "t": "Return",
                    "value": {
                        "t": "LitStr",
                        "v": "caught"
                    }
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("obj".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should return "caught" from the catch block
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("caught".to_string()))
    );
}

#[test]
fn test_try_catch_serialization() {
    // Test that try/catch works correctly after serialization/deserialization
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Try",
            "body": {
                "t": "Return",
                "value": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "obj"
                    },
                    "property": "missing"
                }
            },
            "catch_var": "error",
            "catch_body": {
                "t": "Return",
                "value": {
                    "t": "LitStr",
                    "v": "serialized"
                }
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("obj".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
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
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Try",
            "body": {
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
            },
            "catch_var": "e",
            "catch_body": {
                "t": "Return",
                "value": {
                    "t": "LitStr",
                    "v": "caught_await_error"
                }
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("obj".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
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
    let program_json = r#"{
        "t": "Try",
        "body": {
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
        },
        "catch_var": "e",
        "catch_body": {
            "t": "Block",
            "body": [{
                "t": "Return",
                "value": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "obj"
                    },
                    "property": "another_missing"
                }
            }]
        }
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("obj".to_string(), Val::Obj(HashMap::new()));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // The error in the catch handler should propagate to the top level
    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::PROPERTY_NOT_FOUND);
    assert!(err.message.contains("Property 'another_missing' not found"));
}
