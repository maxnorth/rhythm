//! Basic tests for core execution loop
//!
//! Tests for Milestone 1: Return statement with literal expressions

use crate::executor::tests::helpers::parse_workflow_and_build_vm;
use crate::executor::{errors, run_until_done, Control, Val};
use maplit::hashmap;
use std::collections::HashMap;

/* ===================== Test Suite ===================== */

#[test]
fn test_return_literal_num() {
    let source = r#"
            return 42
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_return_literal_bool() {
    let source = r#"
            return true
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Bool(true)));
}

#[test]
fn test_return_literal_str() {
    let source = r#"
            return "hello"
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("hello".to_string())));
}

#[test]
fn test_return_null() {
    let source = r#"
            return null
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Null));
}

#[test]
fn test_nested_blocks() {
    // Test nested block statements
    let source = r#"
            {
                {
                    {
                        return 42
                    }
                }
            }    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_return_ctx() {
    let source = r#"
            return Context
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // ctx should be an empty object
    assert_eq!(vm.control, Control::Return(Val::Obj(hashmap! {})));
}

#[test]
fn test_return_inputs() {
    let source = r#"
            return Inputs
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // inputs should be an empty object
    assert_eq!(vm.control, Control::Return(Val::Obj(hashmap! {})));
}

#[test]
fn test_initial_env() {
    let source = r#"
            return Inputs
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
            return Inputs.name
        "#;

    let inputs = hashmap! {
        "name".to_string() => Val::Str("Alice".to_string()),
        "age".to_string() => Val::Num(30.0),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    // Should return Inputs.name
    assert_eq!(vm.control, Control::Return(Val::Str("Alice".to_string())));
}

#[test]
fn test_nested_member_access() {
    let source = r#"
            return Inputs.user.id
        "#;

    let inputs = hashmap! {
        "user".to_string() => Val::Obj(hashmap! {
            "id".to_string() => Val::Num(123.0),
            "name".to_string() => Val::Str("Bob".to_string()),
        }),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    // Should return Inputs.user.id
    assert_eq!(vm.control, Control::Return(Val::Num(123.0)));
}

/* ===================== Expression Statement Tests ===================== */

#[test]
fn test_expr_stmt_simple() {
    // Test a simple expression statement (value is discarded)
    let source = r#"
            42
            return "done"
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // Should return "done" (the expr statement result is discarded)
    assert_eq!(vm.control, Control::Return(Val::Str("done".to_string())));
}

#[test]
fn test_expr_stmt_with_member_access() {
    // Test expression statement with member access
    let source = r#"
            Inputs.value
            return 999
        "#;

    let inputs = hashmap! {
        "value".to_string() => Val::Str("ignored".to_string()),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    // Should return 999 (the expr statement result is discarded)
    assert_eq!(vm.control, Control::Return(Val::Num(999.0)));
}

#[test]
fn test_expr_stmt_error_propagates() {
    // Test that errors in expression statements propagate correctly
    let source = r#"
            Inputs.missing
            return "should_not_reach"
        "#;

    let inputs = hashmap! {};

    let mut vm = parse_workflow_and_build_vm(source, inputs);
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
            return 42
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_workflow_return_string() {
    let source = r#"
            return "hello world"
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
            return Inputs
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
            return Inputs.userId
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
            return Inputs.user.name
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
            return Inputs.value
        "#;

    let inputs = hashmap! {
        "value".to_string() => Val::Str("custom".to_string()),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("custom".to_string())));
}

#[test]
fn test_workflow_multiline_body() {
    let source = r#"
            return 1
            return 2
            return 3
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    // Should return from first return statement
    assert_eq!(vm.control, Control::Return(Val::Num(1.0)));
}

/* ===================== Function Call Syntax Tests ===================== */

#[test]
fn test_call_empty_args() {
    // Test that empty parentheses parse correctly
    let source = r#"
            return Math.floor()
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // Should error (wrong arg count), but the syntax should parse
    assert!(matches!(vm.control, Control::Throw(_)));
}

#[test]
fn test_call_single_arg() {
    let source = r#"
            return Math.floor(3.7)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}

#[test]
fn test_call_multiple_args() {
    let source = r#"
            return add(10, 32)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_call_nested() {
    let source = r#"
            return Math.floor(add(10.5, 5.7))
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // add(10.5, 5.7) = 16.2, Math.floor(16.2) = 16.0
    assert_eq!(vm.control, Control::Return(Val::Num(16.0)));
}

#[test]
fn test_call_with_member_access_arg() {
    let source = r#"
            return Math.floor(Inputs.value)
        "#;

    let inputs = hashmap! {
        "value".to_string() => Val::Num(9.8),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(9.0)));
}

#[test]
fn test_call_method_style() {
    // Test calling methods on objects (Math.floor style)
    let source = r#"
            return Math.ceil(4.2)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(5.0)));
}

/* ===================== Await Expression Syntax Tests ===================== */

#[test]
fn test_await_task_creation() {
    // Test basic await syntax with task creation
    let source = r#"
            return await Task.run("test_task", Inputs)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // Should suspend on the task
    assert!(matches!(vm.control, Control::Suspend(_)));
}

#[test]
fn test_await_with_member_access() {
    // Test await with member access expression (Task.run)
    let source = r#"
            return await Task.run("another_task", Inputs)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // Should suspend on the task
    assert!(matches!(vm.control, Control::Suspend(_)));
}

#[test]
fn test_await_non_task_value() {
    // Test that awaiting a non-task value returns the value (like JavaScript)
    let source = r#"
            return await Math.floor(3.7)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // Math.floor returns a number, not a task, so await just returns it
    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}

#[test]
fn test_expression_without_await() {
    // Test that task creation without await returns the Task value
    let source = r#"
            return Task.run("test_task", Inputs)
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // Should return a Promise(Task) value (not suspend)
    // Task ID is a UUID, so we can't predict it - just verify it's a Promise(Task)
    match vm.control {
        Control::Return(Val::Promise(crate::executor::Awaitable::Task(_task_id))) => {
            // Success - we got a Promise(Task) value
        }
        _ => panic!(
            "Expected Return with Promise(Task) value, got {:?}",
            vm.control
        ),
    }
}

/* ===================== Optional Main Function Wrapper Tests ===================== */

#[test]
fn test_main_function_wrapper_executes() {
    // Test that optional async function main() wrapper works end-to-end
    let source = r#"
        async function main() {
            return 42
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_main_function_wrapper_with_inputs() {
    // Test main wrapper with inputs access
    let source = r#"
        async function main() {
            return Inputs.value
        }
    "#;

    let inputs = hashmap! {
        "value".to_string() => Val::Str("from_input".to_string()),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);
    assert_eq!(
        vm.control,
        Control::Return(Val::Str("from_input".to_string()))
    );
}

#[test]
fn test_main_function_wrapper_with_variables() {
    // Test main wrapper with variable declarations
    let source = r#"
        async function main() {
            let x = 10
            let y = 32
            return x + y
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

/* ===================== Bare Statement Execution Tests (Testing Only) ===================== */

#[test]
fn test_execute_bare_return_number() {
    let source = r#"
            return 42
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_execute_bare_return_string() {
    let source = r#"
            return "test"
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Str("test".to_string())));
}

#[test]
fn test_execute_bare_identifier() {
    let source = r#"
            return Inputs.x
        "#;

    let inputs = hashmap! {
        "x".to_string() => Val::Num(42.0),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_execute_bare_member_access() {
    let source = r#"
            return Inputs.userId
        "#;

    let inputs = hashmap! {
        "userId".to_string() => Val::Num(789.0),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);
    assert_eq!(vm.control, Control::Return(Val::Num(789.0)));
}
