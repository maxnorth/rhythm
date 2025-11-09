//! Basic tests for core execution loop
//!
//! Tests for Milestone 1: Return statement with literal expressions

use crate::interpreter::executor_v2::{errors, run_until_done, Control, Stmt, Val, VM};
use crate::interpreter::parser_v2::{self, WorkflowDef};
use maplit::hashmap;
use std::collections::HashMap;

/* ===================== Test Helpers ===================== */

/// Helper: Parse workflow, validate, serialize/deserialize, and create VM
///
/// This helper does the full pipeline:
/// 1. Parse workflow source
/// 2. Validate workflow (semantic validation)
/// 3. Round-trip through JSON (verify serialization)
/// 4. Bind parameters to environment (ctx, inputs convention)
/// 5. Create and return VM
///
/// The test can then execute the VM, check results, etc.
fn parse_workflow_and_build_vm(source: &str, inputs: HashMap<String, Val>) -> VM {
    // Parse workflow (enforces wrapper requirement)
    let workflow = parser_v2::parse_workflow(source).expect("Parse workflow failed");

    // Validate workflow (semantic validation)
    parser_v2::semantic_validator::validate_workflow(&workflow)
        .expect("Workflow validation failed");

    // Round-trip through JSON to verify serialization works
    let json = serde_json::to_string(&workflow).expect("Workflow serialization failed");
    let workflow: WorkflowDef =
        serde_json::from_str(&json).expect("Workflow deserialization failed");

    // Build environment based on workflow parameters
    // Convention: first param = ctx, second param = inputs
    let mut env = HashMap::new();
    if workflow.params.len() >= 1 {
        env.insert(workflow.params[0].clone(), Val::Obj(HashMap::new())); // ctx - empty by default
    }
    if workflow.params.len() >= 2 {
        env.insert(workflow.params[1].clone(), Val::Obj(inputs));
    }

    // Create VM from workflow body
    VM::new(workflow.body.clone(), env)
}

/// Helper: Parse bare statement, serialize/deserialize, and create VM (for testing parser internals)
///
/// This helper:
/// 1. Parses a bare statement (bypasses workflow wrapper requirement)
/// 2. Wraps in Block as executor expects
/// 3. Round-trips through JSON
/// 4. Creates VM with given environment
///
/// This is for testing parser internals. Production code should use parse_workflow_and_build_vm.
fn parse_statement_and_build_vm(source: &str, env: HashMap<String, Val>) -> VM {
    // Parse source code
    let ast = parser_v2::parse(source).expect("Parse failed");

    // Wrap in Block as executor expects
    let program = Stmt::Block { body: vec![ast] };

    // Round-trip through JSON to verify serialization works
    let json = serde_json::to_string(&program).expect("Serialization failed");
    let program: Stmt = serde_json::from_str(&json).expect("Deserialization failed");

    // Create and return VM
    VM::new(program, env)
}

#[test]
fn test_return_literal_num() {
    let source = r#"
        async function workflow(ctx) {
            return 42
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_return_literal_bool() {
    let source = r#"
        async function workflow(ctx) {
            return true
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_return_literal_str() {
    let source = r#"
        async function workflow(ctx) {
            return "hello"
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::Str("hello".to_string()))
    );
}

#[test]
fn test_return_null() {
    let source = r#"
        async function workflow(ctx) {
            return null
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Null));
}

#[test]
fn test_nested_blocks() {
    // Note: Parser doesn't support bare block statements yet, using JSON
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Block",
            "body": [{
                "t": "Block",
                "body": [{
                    "t": "Return",
                    "value": {
                        "t": "LitNum",
                        "v": 42.0
                    }
                }]
            }]
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_return_ctx() {
    let source = r#"
        async function workflow(ctx) {
            return ctx
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // ctx should be an empty object
    assert_eq!(vm.control, Control::Return(Val::Obj(hashmap! {})));
}

#[test]
fn test_return_inputs() {
    let source = r#"
        async function workflow(ctx, inputs) {
            return inputs
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // inputs should be an empty object
    assert_eq!(
        vm.control,
        Control::Return(Val::Obj(hashmap! {}))
    );
}

#[test]
fn test_initial_env() {
    let source = r#"
        async function workflow(ctx, inputs) {
            return inputs
        }
    "#;

    let inputs = hashmap! {
        "name".to_string() => Val::Str("Alice".to_string()),
        "age".to_string() => Val::Num(30.0),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs.clone());
    run_until_done(&mut vm);

    // Should return the inputs object we provided
    assert_eq!(vm.control, Control::Return(Val::Obj(inputs)));
}

#[test]
fn test_member_access() {
    let source = r#"
        async function workflow(ctx, inputs) {
            return inputs.name
        }
    "#;

    let inputs = hashmap! {
        "name".to_string() => Val::Str("Alice".to_string()),
        "age".to_string() => Val::Num(30.0),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    // Should return inputs.name
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("Alice".to_string()))
    );
}

#[test]
fn test_nested_member_access() {
    let source = r#"
        async function workflow(ctx, inputs) {
            return inputs.user.id
        }
    "#;

    let inputs = hashmap! {
        "user".to_string() => Val::Obj(hashmap! {
            "id".to_string() => Val::Num(123.0),
            "name".to_string() => Val::Str("Bob".to_string()),
        }),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    // Should return inputs.user.id
    assert_eq!(vm.control, Control::Return(Val::Num(123.0)));
}

/* ===================== Expression Statement Tests ===================== */

#[test]
fn test_expr_stmt_simple() {
    // Note: Parser doesn't support expression statements yet, using JSON
    // Test a simple expression statement (value is discarded)
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Expr",
                "expr": {
                    "t": "LitNum",
                    "v": 42.0
                }
            },
            {
                "t": "Return",
                "value": {
                    "t": "LitStr",
                    "v": "done"
                }
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    // Should return "done" (the expr statement result is discarded)
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("done".to_string()))
    );
}

#[test]
fn test_expr_stmt_with_member_access() {
    // Note: Parser doesn't support expression statements yet, using JSON
    // Test expression statement with member access
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Expr",
                "expr": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "obj"
                    },
                    "property": "value"
                }
            },
            {
                "t": "Return",
                "value": {
                    "t": "LitNum",
                    "v": 999.0
                }
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let env = hashmap! {
        "obj".to_string() => Val::Obj(hashmap! {
            "value".to_string() => Val::Str("ignored".to_string()),
        }),
    };

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should return 999 (the expr statement result is discarded)
    assert_eq!(vm.control, Control::Return(Val::Num(999.0)));
}

#[test]
fn test_expr_stmt_error_propagates() {
    // Note: Parser doesn't support expression statements yet, using JSON
    // Test that errors in expression statements propagate correctly
    let program_json = r#"{
        "t": "Block",
        "body": [
            {
                "t": "Expr",
                "expr": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "obj"
                    },
                    "property": "missing"
                }
            },
            {
                "t": "Return",
                "value": {
                    "t": "LitStr",
                    "v": "should_not_reach"
                }
            }
        ]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let env = hashmap! {
        "obj".to_string() => Val::Obj(hashmap! {}),
    };

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    // Should throw an error
    let Control::Throw(Val::Error(err)) = vm.control else {
        unreachable!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::PROPERTY_NOT_FOUND);
    assert!(err.message.contains("Property 'missing' not found"));
}

/* ===================== Workflow Syntax Tests ===================== */

#[test]
fn test_workflow_return_number() {
    let source = r#"
        async function workflow(ctx) {
            return 42
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_workflow_return_string() {
    let source = r#"
        async function workflow(ctx) {
            return "hello world"
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("hello world".to_string()))
    );
}

#[test]
fn test_workflow_access_inputs() {
    let source = r#"
        async function workflow(ctx, inputs) {
            return inputs
        }
    "#;

    let inputs = hashmap! {
        "userId".to_string() => Val::Num(123.0),
        "userName".to_string() => Val::Str("Alice".to_string()),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs.clone());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Obj(inputs)));
}

#[test]
fn test_workflow_member_access() {
    let source = r#"
        async function workflow(ctx, inputs) {
            return inputs.userId
        }
    "#;

    let inputs = hashmap! {
        "userId".to_string() => Val::Num(999.0),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(999.0)));
}

#[test]
fn test_workflow_nested_member_access() {
    let source = r#"
        async function workflow(ctx, inputs) {
            return inputs.user.name
        }
    "#;

    let inputs = hashmap! {
        "user".to_string() => Val::Obj(hashmap! {
            "name".to_string() => Val::Str("Bob".to_string()),
            "id".to_string() => Val::Num(456.0),
        }),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("Bob".to_string())));
}

#[test]
fn test_workflow_custom_param_names() {
    let source = r#"
        async function workflow(context, data) {
            return data.value
        }
    "#;

    let inputs = hashmap! {
        "value".to_string() => Val::Str("custom".to_string()),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("custom".to_string()))
    );
}

#[test]
fn test_workflow_multiline_body() {
    let source = r#"
        async function workflow(ctx) {
            return 1
            return 2
            return 3
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    // Should return from first return statement
    assert_eq!(vm.control, Control::Return(Val::Num(1.0)));
}

/* ===================== Bare Statement Execution Tests (Testing Only) ===================== */

#[test]
fn test_execute_bare_return_number() {
    let mut vm = parse_statement_and_build_vm("return 42", HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_execute_bare_return_string() {
    let mut vm = parse_statement_and_build_vm(r#"return "test""#, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("test".to_string()))
    );
}

#[test]
fn test_execute_bare_identifier() {
    let env = hashmap! {
        "x".to_string() => Val::Num(42.0),
    };

    let mut vm = parse_statement_and_build_vm("return x", env);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_execute_bare_member_access() {
    let env = hashmap! {
        "inputs".to_string() => Val::Obj(hashmap! {
            "userId".to_string() => Val::Num(789.0),
        }),
    };

    let mut vm = parse_statement_and_build_vm("return inputs.userId", env);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(789.0)));
}
