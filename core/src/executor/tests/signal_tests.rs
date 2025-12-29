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

/// Test that demonstrates the try/catch scoping issue with signal resumption
///
/// This test reproduces the bug where:
/// 1. Variable is declared in try block
/// 2. Signal.next is awaited (VM suspends)
/// 3. VM is serialized/deserialized (simulating persistence)
/// 4. VM resumes with signal value
/// 5. After resumption, an error occurs
/// 6. Catch block tries to access the try-block variable -> FAILS with undefined
///
/// This is expected behavior - try and catch have separate block scopes.
/// Variables declared in try are NOT accessible in catch.
#[test]
fn test_signal_resume_try_catch_scoping() {
    // Workflow that declares a variable in try, awaits signal, then throws
    let source = r#"
        let result = null
        try {
            let user_email = "test@example.com"
            let signal_data = await Signal.next("approval")
            // After resumption, access missing property to trigger error
            return signal_data.missing_property
        } catch (e) {
            // Try to use user_email from try block - THIS WILL FAIL
            result = user_email
        }
        return result
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should suspend waiting for signal
    assert!(
        matches!(&vm.control, Control::Suspend(Awaitable::Signal { .. })),
        "Expected Suspend(Signal), got {:?}",
        vm.control
    );

    // Verify user_email is in the environment while suspended
    assert!(
        vm.env.contains_key("user_email"),
        "user_email should be in env while suspended in try block"
    );

    // Serialize and deserialize (simulating persistence)
    let serialized = serde_json::to_string(&vm).unwrap();
    let mut vm: VM = serde_json::from_str(&serialized).unwrap();

    // Verify user_email is still in env after deserialization
    assert!(
        vm.env.contains_key("user_email"),
        "user_email should persist through serialization"
    );

    // Resume with signal data (an object without 'missing_property')
    vm.resume(Val::Obj(HashMap::new()));
    run_until_done(&mut vm);

    // The workflow should fail with "Undefined variable 'user_email'"
    // because user_email was declared in try block and is not accessible in catch
    let Control::Throw(Val::Error(err)) = &vm.control else {
        panic!(
            "Expected Throw with undefined variable error, got {:?}",
            vm.control
        );
    };
    assert!(
        err.message.contains("Undefined variable"),
        "Expected undefined variable error, got: {}",
        err.message
    );
    assert!(
        err.message.contains("user_email"),
        "Error should mention user_email, got: {}",
        err.message
    );
}

/// Test that demonstrates the CORRECT way to handle this - declare outside try
#[test]
fn test_signal_resume_try_catch_variable_outside() {
    // Workflow that declares variable OUTSIDE try block
    let source = r#"
        let user_email = "test@example.com"
        let result = null
        try {
            let signal_data = await Signal.next("approval")
            return signal_data.missing_property
        } catch (e) {
            // user_email IS accessible because it's declared outside try
            result = user_email
        }
        return result
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should suspend waiting for signal
    assert!(matches!(
        &vm.control,
        Control::Suspend(Awaitable::Signal { .. })
    ));

    // Resume with signal data
    vm.resume(Val::Obj(HashMap::new()));
    run_until_done(&mut vm);

    // Should successfully return user_email from catch block
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("test@example.com".to_string()))
    );
}
