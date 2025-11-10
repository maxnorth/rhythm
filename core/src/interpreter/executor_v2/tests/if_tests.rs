//! Tests for If statements

use super::super::*;
use crate::interpreter::parser_v2::{self, WorkflowDef};
use maplit::hashmap;
use std::collections::HashMap;

/* ===================== Test Helper ===================== */

/// Helper: Parse workflow, validate, serialize/deserialize, and create VM
fn parse_workflow_and_build_vm(source: &str, inputs: HashMap<String, Val>) -> VM {
    let workflow = parser_v2::parse_workflow(source).expect("Parse workflow failed");
    parser_v2::semantic_validator::validate_workflow(&workflow)
        .expect("Workflow validation failed");
    let json = serde_json::to_string(&workflow).expect("Workflow serialization failed");
    let workflow: WorkflowDef =
        serde_json::from_str(&json).expect("Workflow deserialization failed");

    let mut env = HashMap::new();
    if workflow.params.len() >= 1 {
        env.insert(workflow.params[0].clone(), Val::Obj(HashMap::new()));
    }
    if workflow.params.len() >= 2 {
        env.insert(workflow.params[1].clone(), Val::Obj(inputs));
    }

    VM::new(workflow.body.clone(), env)
}

#[test]
fn test_if_true_no_else() {
    let source = r#"
        async function workflow(ctx) {
            if (true) {
                return 42
            }
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_if_false_no_else() {
    let source = r#"
        async function workflow(ctx) {
            if (false) {
                return 42
            }
            return 99
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(99.0)));
}

#[test]
fn test_if_true_with_else() {
    let source = r#"
        async function workflow(ctx) {
            if (true) {
                return 42
            } else {
                return 99
            }
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_if_false_with_else() {
    let source = r#"
        async function workflow(ctx) {
            if (false) {
                return 42
            } else {
                return 99
            }
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(99.0)));
}

#[test]
fn test_if_truthiness_number() {
    // if (42) { return "truthy"; } else { return "falsy"; }
    let program_json = r#"{
        "t": "If",
        "test": {"t": "LitNum", "v": 42.0},
        "then_s": {
            "t": "Return",
            "value": {"t": "LitStr", "v": "truthy"}
        },
        "else_s": {
            "t": "Return",
            "value": {"t": "LitStr", "v": "falsy"}
        }
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::Str("truthy".to_string()))
    );
}

#[test]
fn test_if_truthiness_false() {
    // if (false) { return "truthy"; } else { return "falsy"; }
    let program_json = r#"{
        "t": "If",
        "test": {"t": "LitBool", "v": false},
        "then_s": {
            "t": "Return",
            "value": {"t": "LitStr", "v": "truthy"}
        },
        "else_s": {
            "t": "Return",
            "value": {"t": "LitStr", "v": "falsy"}
        }
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("falsy".to_string())));
}

#[test]
fn test_if_with_variable_test() {
    // x = true; if (x) { return "yes"; } else { return "no"; }
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "path": [],
                "var": "x",
                "value": {"t": "LitBool", "v": true}
            },
            {
                "t": "If",
                "test": {"t": "Ident", "name": "x"},
                "then_s": {
                    "t": "Return",
                    "value": {"t": "LitStr", "v": "yes"}
                },
                "else_s": {
                    "t": "Return",
                    "value": {"t": "LitStr", "v": "no"}
                }
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("yes".to_string())));
}

#[test]
fn test_if_with_assignment_in_branch() {
    // x = 1; if (true) { x = 42; } return x;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "path": [],
                "var": "x",
                "value": {"t": "LitNum", "v": 1.0}
            },
            {
                "t": "If",
                "test": {"t": "LitBool", "v": true},
                "then_s": {
                    "t": "Assign",
                    "path": [],
                    "var": "x",
                    "value": {"t": "LitNum", "v": 42.0}
                },
                "else_s": null
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "x"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
    assert_eq!(vm.env.get("x"), Some(&Val::Num(42.0)));
}

#[test]
fn test_if_nested() {
    // if (true) { if (false) { return 1; } else { return 2; } } else { return 3; }
    let program_json = r#"{
        "t": "If",
        "test": {"t": "LitBool", "v": true},
        "then_s": {
            "t": "If",
            "test": {"t": "LitBool", "v": false},
            "then_s": {
                "t": "Return",
                "value": {"t": "LitNum", "v": 1.0}
            },
            "else_s": {
                "t": "Return",
                "value": {"t": "LitNum", "v": 2.0}
            }
        },
        "else_s": {
            "t": "Return",
            "value": {"t": "LitNum", "v": 3.0}
        }
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(2.0)));
}

#[test]
fn test_if_with_block_statement() {
    // if (true) { x = 1; x = 2; return x; }
    let program_json = r#"{
        "t": "If",
        "test": {"t": "LitBool", "v": true},
        "then_s": {
            "t": "Block",
            "body": [
                {
                    "t": "Assign",
                    "path": [],
                    "var": "x",
                    "value": {"t": "LitNum", "v": 1.0}
                },
                {
                    "t": "Assign",
                    "path": [],
                    "var": "x",
                    "value": {"t": "LitNum", "v": 2.0}
                },
                {
                    "t": "Return",
                    "value": {"t": "Ident", "name": "x"}
                }
            ]
        },
        "else_s": null
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(2.0)));
}

#[test]
fn test_if_with_error_in_test() {
    // if (ctx.bad) { return 1; }
    let program_json = r#"{
        "t": "If",
        "test": {
            "t": "Member",
            "object": {"t": "Ident", "name": "ctx"},
            "property": "bad"
        },
        "then_s": {
            "t": "Return",
            "value": {"t": "LitNum", "v": 1.0}
        },
        "else_s": null
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
fn test_if_with_try_catch() {
    // result = "not_set"; if (true) { try { throw {code: "E", message: "msg"}; } catch (e) { result = "caught"; } } return result;
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Assign",
                "path": [],
                "var": "result",
                "value": {"t": "LitStr", "v": "not_set"}
            },
            {
                "t": "If",
                "test": {"t": "LitBool", "v": true},
                "then_s": {
                    "t": "Try",
                    "body": {
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
                    "catch_var": "e",
                    "catch_body": {
                        "t": "Assign",
                        "path": [],
                        "var": "result",
                        "value": {"t": "LitStr", "v": "caught"}
                    }
                },
                "else_s": null
            },
            {
                "t": "Return",
                "value": {"t": "Ident", "name": "result"}
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::Str("caught".to_string()))
    );
}

/* ===================== Else-If Chain Tests ===================== */

#[test]
fn test_else_if_chain() {
    // Test else-if chain: if/else if/else
    let source = r#"
        async function workflow(ctx, inputs) {
            if (inputs.value) {
                return "first"
            } else if (inputs.fallback) {
                return "second"
            } else {
                return "third"
            }
        }
    "#;

    // Test first branch
    let inputs = hashmap! {
        "value".to_string() => Val::Bool(true),
        "fallback".to_string() => Val::Bool(false),
    };
    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("first".to_string()))
    );

    // Test second branch (else-if)
    let inputs = hashmap! {
        "value".to_string() => Val::Bool(false),
        "fallback".to_string() => Val::Bool(true),
    };
    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("second".to_string()))
    );

    // Test third branch (else)
    let inputs = hashmap! {
        "value".to_string() => Val::Bool(false),
        "fallback".to_string() => Val::Bool(false),
    };
    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("third".to_string()))
    );
}

#[test]
fn test_multiple_else_if() {
    // Test multiple else-if clauses
    let source = r#"
        async function workflow(ctx, inputs) {
            if (inputs.a) {
                return 1
            } else if (inputs.b) {
                return 2
            } else if (inputs.c) {
                return 3
            } else {
                return 4
            }
        }
    "#;

    // Test third branch (c)
    let inputs = hashmap! {
        "a".to_string() => Val::Bool(false),
        "b".to_string() => Val::Bool(false),
        "c".to_string() => Val::Bool(true),
    };
    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}
