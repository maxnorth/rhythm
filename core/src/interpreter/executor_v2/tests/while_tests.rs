//! Tests for While statements

use super::super::*;
use std::collections::HashMap;

#[test]
fn test_while_simple_loop() {
    // i = 0; while (i < 3) { i = i + 1; } return i;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "path": [],
                "var": "i",
                "value": {"t": "LitNum", "v": 0.0}
            },
            {
                "t": "While",
                "test": {
                    "t": "Call",
                    "callee": {"t": "Ident", "name": "lt"},
                    "args": [
                        {"t": "Ident", "name": "i"},
                        {"t": "LitNum", "v": 3.0}
                    ]
                },
                "body": {
                    "t": "Assign",
                    "path": [],
                    "var": "i",
                    "value": {
                        "t": "Call",
                        "callee": {"t": "Ident", "name": "add"},
                        "args": [
                            {"t": "Ident", "name": "i"},
                            {"t": "LitNum", "v": 1.0}
                        ]
                    }
                }
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "i"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
    assert_eq!(vm.env.get("i"), Some(&Val::Num(3.0)));
}

#[test]
fn test_while_zero_iterations() {
    // while (false) { return 1; } return 2;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "While",
                "test": {"t": "LitBool", "v": false},
                "body": {
                    "t": "Return",
                    "value": {"t": "LitNum", "v": 1.0}
                }
            },
            {
                "t": "Return",
                "value": {"t": "LitNum", "v": 2.0}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(2.0)));
}

#[test]
fn test_while_with_break() {
    // i = 0; while (true) { if (i >= 5) { break; } i = i + 1; } return i;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "path": [],
                "var": "i",
                "value": {"t": "LitNum", "v": 0.0}
            },
            {
                "t": "While",
                "test": {"t": "LitBool", "v": true},
                "body": {
                    "t": "Block",
                    "body": [
                        {
                            "t": "If",
                            "test": {
                                "t": "Call",
                                "callee": {"t": "Ident", "name": "gte"},
                                "args": [
                                    {"t": "Ident", "name": "i"},
                                    {"t": "LitNum", "v": 5.0}
                                ]
                            },
                            "then_s": {"t": "Break"},
                            "else_s": null
                        },
                        {
                            "t": "Assign",
                            "path": [],
                            "var": "i",
                            "value": {
                                "t": "Call",
                                "callee": {"t": "Ident", "name": "add"},
                                "args": [
                                    {"t": "Ident", "name": "i"},
                                    {"t": "LitNum", "v": 1.0}
                                ]
                            }
                        }
                    ]
                }
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "i"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(5.0)));
}

#[test]
fn test_while_with_continue() {
    // i = 0; sum = 0; while (i < 5) { i = i + 1; if (i == 3) { continue; } sum = sum + i; } return sum;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "path": [],
                "var": "i",
                "value": {"t": "LitNum", "v": 0.0}
            },
            {
                "t": "Assign",
                "path": [],
                "var": "sum",
                "value": {"t": "LitNum", "v": 0.0}
            },
            {
                "t": "While",
                "test": {
                    "t": "Call",
                    "callee": {"t": "Ident", "name": "lt"},
                    "args": [
                        {"t": "Ident", "name": "i"},
                        {"t": "LitNum", "v": 5.0}
                    ]
                },
                "body": {
                    "t": "Block",
                    "body": [
                        {
                            "t": "Assign",
                            "path": [],
                            "var": "i",
                            "value": {
                                "t": "Call",
                                "callee": {"t": "Ident", "name": "add"},
                                "args": [
                                    {"t": "Ident", "name": "i"},
                                    {"t": "LitNum", "v": 1.0}
                                ]
                            }
                        },
                        {
                            "t": "If",
                            "test": {
                                "t": "Call",
                                "callee": {"t": "Ident", "name": "eq"},
                                "args": [
                                    {"t": "Ident", "name": "i"},
                                    {"t": "LitNum", "v": 3.0}
                                ]
                            },
                            "then_s": {"t": "Continue"},
                            "else_s": null
                        },
                        {
                            "t": "Assign",
                            "path": [],
                            "var": "sum",
                            "value": {
                                "t": "Call",
                                "callee": {"t": "Ident", "name": "add"},
                                "args": [
                                    {"t": "Ident", "name": "sum"},
                                    {"t": "Ident", "name": "i"}
                                ]
                            }
                        }
                    ]
                }
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "sum"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    // sum = 1 + 2 + 4 + 5 = 12 (skipping 3)
    assert_eq!(vm.control, Control::Return(Val::Num(12.0)));
}

#[test]
fn test_while_nested() {
    // i = 0; j = 0; while (i < 2) { while (j < 2) { j = j + 1; } i = i + 1; j = 0; } return i;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "path": [],
                "var": "i",
                "value": {"t": "LitNum", "v": 0.0}
            },
            {
                "t": "Assign",
                "path": [],
                "var": "j",
                "value": {"t": "LitNum", "v": 0.0}
            },
            {
                "t": "While",
                "test": {
                    "t": "Call",
                    "callee": {"t": "Ident", "name": "lt"},
                    "args": [
                        {"t": "Ident", "name": "i"},
                        {"t": "LitNum", "v": 2.0}
                    ]
                },
                "body": {
                    "t": "Block",
                    "body": [
                        {
                            "t": "While",
                            "test": {
                                "t": "Call",
                                "callee": {"t": "Ident", "name": "lt"},
                                "args": [
                                    {"t": "Ident", "name": "j"},
                                    {"t": "LitNum", "v": 2.0}
                                ]
                            },
                            "body": {
                                "t": "Assign",
                                "path": [],
                                "var": "j",
                                "value": {
                                    "t": "Call",
                                    "callee": {"t": "Ident", "name": "add"},
                                    "args": [
                                        {"t": "Ident", "name": "j"},
                                        {"t": "LitNum", "v": 1.0}
                                    ]
                                }
                            }
                        },
                        {
                            "t": "Assign",
                            "path": [],
                            "var": "i",
                            "value": {
                                "t": "Call",
                                "callee": {"t": "Ident", "name": "add"},
                                "args": [
                                    {"t": "Ident", "name": "i"},
                                    {"t": "LitNum", "v": 1.0}
                                ]
                            }
                        },
                        {
                            "t": "Assign",
                            "path": [],
                            "var": "j",
                            "value": {"t": "LitNum", "v": 0.0}
                        }
                    ]
                }
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "i"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(2.0)));
}

#[test]
fn test_while_with_return() {
    // i = 0; while (i < 10) { i = i + 1; if (i == 5) { return i; } } return 99;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "path": [],
                "var": "i",
                "value": {"t": "LitNum", "v": 0.0}
            },
            {
                "t": "While",
                "test": {
                    "t": "Call",
                    "callee": {"t": "Ident", "name": "lt"},
                    "args": [
                        {"t": "Ident", "name": "i"},
                        {"t": "LitNum", "v": 10.0}
                    ]
                },
                "body": {
                    "t": "Block",
                    "body": [
                        {
                            "t": "Assign",
                            "path": [],
                            "var": "i",
                            "value": {
                                "t": "Call",
                                "callee": {"t": "Ident", "name": "add"},
                                "args": [
                                    {"t": "Ident", "name": "i"},
                                    {"t": "LitNum", "v": 1.0}
                                ]
                            }
                        },
                        {
                            "t": "If",
                            "test": {
                                "t": "Call",
                                "callee": {"t": "Ident", "name": "eq"},
                                "args": [
                                    {"t": "Ident", "name": "i"},
                                    {"t": "LitNum", "v": 5.0}
                                ]
                            },
                            "then_s": {
                                "t": "Return",
                                "value": {"t": "Ident", "name": "i"}
                            },
                            "else_s": null
                        }
                    ]
                }
            },
            {
                "t": "Return",
                "value": {"t": "LitNum", "v": 99.0}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(5.0)));
}

#[test]
fn test_while_with_error_in_test() {
    // while (ctx.bad) { return 1; }
    let program_json = r#"{
        "t": "While",
        "test": {
            "t": "Member",
            "object": {"t": "Ident", "name": "ctx"},
            "property": "bad"
        },
        "body": {
            "t": "Return",
            "value": {"t": "LitNum", "v": 1.0}
        }
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    // Should throw an error
    match &vm.control {
        Control::Throw(Val::Error(err)) => {
            // Expression evaluator throws INTERNAL_ERROR for undefined variables
            assert_eq!(err.code, "INTERNAL_ERROR");
        }
        _ => panic!("Expected error, got: {:?}", vm.control),
    }
}

#[test]
fn test_while_with_try_catch() {
    // i = 0; while (i < 5) { try { if (i == 3) { throw {code: \"E\", message: \"msg\"}; } i = i + 1; } catch (e) { i = 10; } } return i;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "path": [],
                "var": "i",
                "value": {"t": "LitNum", "v": 0.0}
            },
            {
                "t": "While",
                "test": {
                    "t": "Call",
                    "callee": {"t": "Ident", "name": "lt"},
                    "args": [
                        {"t": "Ident", "name": "i"},
                        {"t": "LitNum", "v": 5.0}
                    ]
                },
                "body": {
                    "t": "Try",
                    "body": {
                        "t": "Block",
                        "body": [
                            {
                                "t": "If",
                                "test": {
                                    "t": "Call",
                                    "callee": {"t": "Ident", "name": "eq"},
                                    "args": [
                                        {"t": "Ident", "name": "i"},
                                        {"t": "LitNum", "v": 3.0}
                                    ]
                                },
                                "then_s": {
                                    "t": "Expr",
                                    "expr": {
                                        "t": "Call",
                                        "callee": {"t": "Ident", "name": "throw"},
                                        "args": [
                                            {
                                                "t": "LitObj",
                                                "properties": [
                                                    ["code", {"t": "LitStr", "v": "E"}],
                                                    ["message", {"t": "LitStr", "v": "msg"}]
                                                ]
                                            }
                                        ]
                                    }
                                },
                                "else_s": null
                            },
                            {
                                "t": "Assign",
                                "path": [],
                                "var": "i",
                                "value": {
                                    "t": "Call",
                                    "callee": {"t": "Ident", "name": "add"},
                                    "args": [
                                        {"t": "Ident", "name": "i"},
                                        {"t": "LitNum", "v": 1.0}
                                    ]
                                }
                            }
                        ]
                    },
                    "catch_var": "e",
                    "catch_body": {
                        "t": "Assign",
                        "path": [],
                        "var": "i",
                        "value": {"t": "LitNum", "v": 10.0}
                    }
                }
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "i"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    // Loop runs 0,1,2 iterations normally, then throws on i=3, catch sets i=10, loop exits
    assert_eq!(vm.control, Control::Return(Val::Num(10.0)));
}

#[test]
fn test_while_accumulator() {
    // sum = 0; i = 1; while (i <= 5) { sum = sum + i; i = i + 1; } return sum;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "path": [],
                "var": "sum",
                "value": {"t": "LitNum", "v": 0.0}
            },
            {
                "t": "Assign",
                "path": [],
                "var": "i",
                "value": {"t": "LitNum", "v": 1.0}
            },
            {
                "t": "While",
                "test": {
                    "t": "Call",
                    "callee": {"t": "Ident", "name": "lte"},
                    "args": [
                        {"t": "Ident", "name": "i"},
                        {"t": "LitNum", "v": 5.0}
                    ]
                },
                "body": {
                    "t": "Block",
                    "body": [
                        {
                            "t": "Assign",
                            "path": [],
                            "var": "sum",
                            "value": {
                                "t": "Call",
                                "callee": {"t": "Ident", "name": "add"},
                                "args": [
                                    {"t": "Ident", "name": "sum"},
                                    {"t": "Ident", "name": "i"}
                                ]
                            }
                        },
                        {
                            "t": "Assign",
                            "path": [],
                            "var": "i",
                            "value": {
                                "t": "Call",
                                "callee": {"t": "Ident", "name": "add"},
                                "args": [
                                    {"t": "Ident", "name": "i"},
                                    {"t": "LitNum", "v": 1.0}
                                ]
                            }
                        }
                    ]
                }
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "sum"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    // sum = 1 + 2 + 3 + 4 + 5 = 15
    assert_eq!(vm.control, Control::Return(Val::Num(15.0)));
}
