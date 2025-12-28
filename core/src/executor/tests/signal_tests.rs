//! Tests for Signal.next() function implementation

use super::helpers::parse_workflow_and_build_vm;
use crate::executor::{errors, run_until_done, Awaitable, Control, Val, VM};
use std::collections::HashMap;

#[test]
fn test_signal_next_returns_promise() {
    let source = r#"
        return Signal.next("approval")
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    match &vm.control {
        Control::Return(Val::Promise(Awaitable::Signal { name, claim_id })) => {
            assert_eq!(name, "approval");
            assert!(!claim_id.is_empty());
        }
        _ => panic!("Expected Promise(Signal), got {:?}", vm.control),
    }
}

#[test]
fn test_await_signal_next_suspends() {
    let source = r#"
        return await Signal.next("approval")
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    match &vm.control {
        Control::Suspend(Awaitable::Signal { name, claim_id }) => {
            assert_eq!(name, "approval");
            assert!(!claim_id.is_empty());
        }
        _ => panic!("Expected Suspend(Signal), got {:?}", vm.control),
    }
}

#[test]
fn test_signal_serialization() {
    let source = r#"
        return await Signal.next("approval")
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let serialized = serde_json::to_string(&vm).unwrap();
    let vm2: VM = serde_json::from_str(&serialized).unwrap();

    match &vm2.control {
        Control::Suspend(Awaitable::Signal { name, .. }) => {
            assert_eq!(name, "approval");
        }
        _ => panic!("Expected Suspend(Signal) after deserialization"),
    }
}

#[test]
fn test_signal_next_wrong_arg_count() {
    let source = r#"return Signal.next()"#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_COUNT);
}

#[test]
fn test_signal_next_wrong_arg_type() {
    let source = r#"return Signal.next(123)"#;
    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_TYPE);
}
