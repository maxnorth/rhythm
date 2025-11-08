//! Tests for standard library functions
//!
//! Tests function calls and Math stdlib

use crate::interpreter::executor_v2::{errors, run_until_done, Control, Stmt, Val, VM};
use std::collections::HashMap;

/* ===================== Math.floor Tests ===================== */

#[test]
fn test_math_floor_basic() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Math"
                    },
                    "property": "floor"
                },
                "args": [{
                    "t": "LitNum",
                    "v": 3.7
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}

#[test]
fn test_math_floor_negative() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Math"
                    },
                    "property": "floor"
                },
                "args": [{
                    "t": "LitNum",
                    "v": -3.2
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(-4.0)));
}

/* ===================== Math.ceil Tests ===================== */

#[test]
fn test_math_ceil_basic() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Math"
                    },
                    "property": "ceil"
                },
                "args": [{
                    "t": "LitNum",
                    "v": 3.2
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(4.0)));
}

/* ===================== Math.abs Tests ===================== */

#[test]
fn test_math_abs_positive() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Math"
                    },
                    "property": "abs"
                },
                "args": [{
                    "t": "LitNum",
                    "v": 5.0
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(5.0)));
}

#[test]
fn test_math_abs_negative() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Math"
                    },
                    "property": "abs"
                },
                "args": [{
                    "t": "LitNum",
                    "v": -5.0
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(5.0)));
}

/* ===================== Math.round Tests ===================== */

#[test]
fn test_math_round_basic() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Math"
                    },
                    "property": "round"
                },
                "args": [{
                    "t": "LitNum",
                    "v": 3.6
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(4.0)));
}

#[test]
fn test_math_round_half() {
    // JavaScript rounds 2.5 to 3 (half-way cases round towards +∞)
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Math"
                    },
                    "property": "round"
                },
                "args": [{
                    "t": "LitNum",
                    "v": 2.5
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}

/* ===================== Error Tests ===================== */

#[test]
fn test_call_not_a_function() {
    // Try to call a number
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "LitNum",
                    "v": 42.0
                },
                "args": []
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::NOT_A_FUNCTION);
    assert!(err.message.contains("not callable"));
}

#[test]
fn test_wrong_arg_count() {
    // Math.floor() with no arguments
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Math"
                    },
                    "property": "floor"
                },
                "args": []
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_COUNT);
    assert!(err.message.contains("Expected 1 argument"));
}

#[test]
fn test_wrong_arg_type() {
    // Math.floor("not a number")
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Math"
                    },
                    "property": "floor"
                },
                "args": [{
                    "t": "LitStr",
                    "v": "not a number"
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_TYPE);
    assert!(err.message.contains("must be a number"));
}

/* ===================== Nested/Complex Tests ===================== */

#[test]
fn test_nested_call() {
    // Math.abs(Math.floor(-3.7)) should return 4.0
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Math"
                    },
                    "property": "abs"
                },
                "args": [{
                    "t": "Call",
                    "callee": {
                        "t": "Member",
                        "object": {
                            "t": "Ident",
                            "name": "Math"
                        },
                        "property": "floor"
                    },
                    "args": [{
                        "t": "LitNum",
                        "v": -3.7
                    }]
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(4.0)));
}

#[test]
fn test_call_with_member_chain() {
    // Math.floor(inputs.value) where inputs.value = 3.7
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Math"
                    },
                    "property": "floor"
                },
                "args": [{
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "inputs"
                    },
                    "property": "value"
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut inputs_obj = HashMap::new();
    inputs_obj.insert("value".to_string(), Val::Num(3.7));

    let mut env = HashMap::new();
    env.insert("inputs".to_string(), Val::Obj(inputs_obj));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}
