//! End-to-end tests: Parse source code and execute with executor_v2
//!
//! These tests verify the full pipeline from source string → AST → execution

use crate::interpreter::executor_v2::{run_until_done, Control, Stmt, Val, VM};
use crate::interpreter::parser_v2;
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
