//! End-to-end tests: Parse source code and execute with executor_v2
//!
//! These tests verify the full pipeline from source string → AST → execution

use crate::interpreter::executor_v2::{run_until_done, Control, Stmt, Val, VM};
use crate::interpreter::parser_v2::{self, WorkflowDef};
use std::collections::HashMap;

/* ===================== Test Helpers ===================== */

/// Test helper: Parse, serialize/deserialize (round-trip), create VM
///
/// This function:
/// 1. Parses the source code
/// 2. Wraps in a Block statement (as executor expects)
/// 3. Serializes to JSON and deserializes back (verifies AST round-tripping)
/// 4. Creates a VM with the given environment
/// 5. Returns the VM ready to be executed
///
/// The test can then run the VM, serialize/resume, test suspension, etc.
fn parse_and_build_vm(source: &str, env: HashMap<String, Val>) -> VM {
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

/// Test helper for workflow functions: Parse workflow, serialize/deserialize, create VM
///
/// This function:
/// 1. Parses the source as a workflow definition
/// 2. Validates that it uses proper workflow wrapper (enforces production rules)
/// 3. Serializes to JSON and deserializes back (verifies WorkflowDef round-tripping)
/// 4. Automatically binds workflow parameters to ctx and inputs:
///    - First param = ctx (empty object by default)
///    - Second param = inputs (provided by caller)
/// 5. Returns both the WorkflowDef and VM
///
/// This establishes the workflow calling convention where workflows receive (ctx, inputs).
fn parse_workflow_and_build_vm(source: &str, inputs: HashMap<String, Val>) -> (WorkflowDef, VM) {
    // Parse workflow (already enforces wrapper requirement)
    let workflow = parser_v2::parse_workflow(source).expect("Parse workflow failed");

    // Validate workflow (semantic validation)
    parser_v2::semantic_validator::validate_workflow(&workflow)
        .expect("Workflow validation failed");

    // Round-trip through JSON to verify serialization works
    let json = serde_json::to_string(&workflow).expect("Workflow serialization failed");
    let workflow: WorkflowDef = serde_json::from_str(&json).expect("Workflow deserialization failed");

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
    let vm = VM::new(workflow.body.clone(), env);

    (workflow, vm)
}

/* ===================== Parse + Execute Tests ===================== */

#[test]
fn test_e2e_return_number() {
    let mut vm = parse_and_build_vm("return 42", HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_e2e_return_negative_number() {
    let mut vm = parse_and_build_vm("return -3.14", HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(-3.14)));
}

#[test]
fn test_e2e_return_boolean_true() {
    let mut vm = parse_and_build_vm("return true", HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_e2e_return_boolean_false() {
    let mut vm = parse_and_build_vm("return false", HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Bool(false)));
}

#[test]
fn test_e2e_return_string() {
    let mut vm = parse_and_build_vm(r#"return "hello world""#, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("hello world".to_string())));
}

#[test]
fn test_e2e_return_empty_string() {
    let mut vm = parse_and_build_vm(r#"return """#, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("".to_string())));
}

#[test]
fn test_e2e_with_whitespace() {
    let mut vm = parse_and_build_vm("   return   42   ", HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_e2e_with_line_comment() {
    let mut vm = parse_and_build_vm("// This is a comment\nreturn 42", HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_e2e_with_block_comment() {
    let mut vm = parse_and_build_vm("/* Block comment */ return 42", HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

/* ===================== Edge Cases ===================== */

#[test]
fn test_e2e_zero() {
    let mut vm = parse_and_build_vm("return 0", HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(0.0)));
}

#[test]
fn test_e2e_decimal_number() {
    let mut vm = parse_and_build_vm("return 123.456", HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(123.456)));
}

#[test]
fn test_e2e_string_with_spaces() {
    let mut vm = parse_and_build_vm(r#"return "hello   world   with   spaces""#, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("hello   world   with   spaces".to_string()))
    );
}

/* ===================== Workflow Function Tests ===================== */

#[test]
fn test_workflow_minimal() {
    // Minimum valid workflow: at least ctx parameter
    let source = r#"
        async function workflow(ctx) {
            return 42
        }
    "#;

    let (workflow, mut vm) = parse_workflow_and_build_vm(source, HashMap::new());

    // Verify workflow definition - has ctx param
    assert_eq!(workflow.params, vec!["ctx"]);

    // Execute
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_workflow_with_ctx_param() {
    let source = r#"
        async function workflow(ctx) {
            return 42
        }
    "#;

    let (workflow, mut vm) = parse_workflow_and_build_vm(source, HashMap::new());

    // Verify parameters - just ctx
    assert_eq!(workflow.params, vec!["ctx"]);

    // Execute
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_workflow_with_ctx_and_inputs() {
    let source = r#"
        async function workflow(ctx, inputs) {
            return 123
        }
    "#;

    let (workflow, mut vm) = parse_workflow_and_build_vm(source, HashMap::new());

    // Verify params - ctx and inputs
    assert_eq!(workflow.params, vec!["ctx", "inputs"]);

    // Execute
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(123.0)));
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

    let (workflow, mut vm) = parse_workflow_and_build_vm(source, HashMap::new());
    assert_eq!(workflow.params, vec!["ctx"]);

    // Execute - should return from first return statement
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(1.0)));
}

/* ===================== Identifier Tests ===================== */

#[test]
fn test_identifier_simple() {
    let mut env = HashMap::new();
    env.insert("x".to_string(), Val::Num(42.0));

    let mut vm = parse_and_build_vm("return x", env);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_identifier_string() {
    let mut env = HashMap::new();
    env.insert("name".to_string(), Val::Str("Alice".to_string()));

    let mut vm = parse_and_build_vm("return name", env);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("Alice".to_string())));
}

#[test]
fn test_identifier_inputs() {
    let mut inputs_obj = HashMap::new();
    inputs_obj.insert("userId".to_string(), Val::Num(123.0));

    let mut env = HashMap::new();
    env.insert("inputs".to_string(), Val::Obj(inputs_obj.clone()));

    let mut vm = parse_and_build_vm("return inputs", env);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Obj(inputs_obj)));
}

/* ===================== Member Access Tests ===================== */

#[test]
fn test_member_access_simple() {
    let mut inputs_obj = HashMap::new();
    inputs_obj.insert("userId".to_string(), Val::Num(123.0));

    let mut env = HashMap::new();
    env.insert("inputs".to_string(), Val::Obj(inputs_obj));

    let mut vm = parse_and_build_vm("return inputs.userId", env);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(123.0)));
}

#[test]
fn test_member_access_string_property() {
    let mut obj = HashMap::new();
    obj.insert("name".to_string(), Val::Str("Bob".to_string()));

    let mut env = HashMap::new();
    env.insert("user".to_string(), Val::Obj(obj));

    let mut vm = parse_and_build_vm("return user.name", env);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("Bob".to_string())));
}

#[test]
fn test_member_access_nested() {
    let mut user_obj = HashMap::new();
    user_obj.insert("id".to_string(), Val::Num(456.0));
    user_obj.insert("name".to_string(), Val::Str("Carol".to_string()));

    let mut ctx_obj = HashMap::new();
    ctx_obj.insert("user".to_string(), Val::Obj(user_obj));

    let mut env = HashMap::new();
    env.insert("ctx".to_string(), Val::Obj(ctx_obj));

    let mut vm = parse_and_build_vm("return ctx.user.id", env);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(456.0)));
}

#[test]
fn test_member_access_deeply_nested() {
    let mut address_obj = HashMap::new();
    address_obj.insert("city".to_string(), Val::Str("NYC".to_string()));

    let mut user_obj = HashMap::new();
    user_obj.insert("address".to_string(), Val::Obj(address_obj));

    let mut ctx_obj = HashMap::new();
    ctx_obj.insert("user".to_string(), Val::Obj(user_obj));

    let mut env = HashMap::new();
    env.insert("ctx".to_string(), Val::Obj(ctx_obj));

    let mut vm = parse_and_build_vm("return ctx.user.address.city", env);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("NYC".to_string())));
}

/* ===================== Workflow with Parameter Access ===================== */

#[test]
fn test_workflow_access_ctx() {
    let source = r#"
        async function workflow(ctx) {
            return ctx
        }
    "#;

    let (workflow, mut vm) = parse_workflow_and_build_vm(source, HashMap::new());
    assert_eq!(workflow.params, vec!["ctx"]);

    run_until_done(&mut vm);
    // ctx is an empty object by default
    assert_eq!(vm.control, Control::Return(Val::Obj(HashMap::new())));
}

#[test]
fn test_workflow_access_inputs() {
    let source = r#"
        async function workflow(ctx, inputs) {
            return inputs
        }
    "#;

    let mut inputs = HashMap::new();
    inputs.insert("userId".to_string(), Val::Num(789.0));
    inputs.insert("userName".to_string(), Val::Str("Dave".to_string()));

    let (workflow, mut vm) = parse_workflow_and_build_vm(source, inputs.clone());
    assert_eq!(workflow.params, vec!["ctx", "inputs"]);

    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Obj(inputs)));
}

#[test]
fn test_workflow_inputs_member_access() {
    let source = r#"
        async function workflow(ctx, inputs) {
            return inputs.userId
        }
    "#;

    let mut inputs = HashMap::new();
    inputs.insert("userId".to_string(), Val::Num(999.0));

    let (workflow, mut vm) = parse_workflow_and_build_vm(source, inputs);
    assert_eq!(workflow.params, vec!["ctx", "inputs"]);

    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(999.0)));
}

#[test]
fn test_workflow_custom_param_names() {
    // Users can name parameters whatever they want
    let source = r#"
        async function workflow(context, data) {
            return data.value
        }
    "#;

    let mut inputs = HashMap::new();
    inputs.insert("value".to_string(), Val::Str("custom".to_string()));

    let (workflow, mut vm) = parse_workflow_and_build_vm(source, inputs);
    assert_eq!(workflow.params, vec!["context", "data"]);

    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("custom".to_string())));
}

/* ===================== Parse Error Tests ===================== */

#[test]
fn test_parser_rejects_bare_statement() {
    // Parser rejects bare statements - must use workflow wrapper
    let source = "return 42";

    // This should fail at parse time
    let result = parser_v2::parse_workflow(source);
    assert!(result.is_err());

    // Error message should mention workflow wrapper requirement
    let err = result.unwrap_err();
    assert!(matches!(err, parser_v2::ParseError::BuildError(_)));
}

#[test]
fn test_parser_accepts_workflow_wrapper() {
    // Parser accepts proper workflow syntax
    let source = r#"
        async function workflow(ctx) {
            return 42
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");
    assert_eq!(workflow.params, vec!["ctx"]);
}

#[test]
fn test_parse_for_testing_allows_bare_statements() {
    // The parse() function (for testing) still allows bare statements
    let source = "return 42";

    let stmt = parser_v2::parse(source).expect("Should parse for testing");

    // Verify it's a return statement
    assert!(matches!(stmt, Stmt::Return { .. }));
}
