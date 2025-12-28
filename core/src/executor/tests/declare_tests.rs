//! Tests for variable declarations (let/const)

use crate::executor::tests::helpers::parse_workflow_and_build_vm;
use crate::executor::{run_until_done, Control, Val};
use maplit::hashmap;

/* ===================== Basic Declaration Tests ===================== */

#[test]
fn test_let_with_number() {
    let source = r#"
            let x = 42
            return x
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_let_with_string() {
    let source = r#"
            let message = "hello"
            return message
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("hello".to_string())));
}

#[test]
fn test_let_without_init() {
    let source = r#"
            let x
            return x
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Null));
}

#[test]
fn test_const_with_value() {
    let source = r#"
            const y = 100
            return y
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(100.0)));
}

#[test]
fn test_const_without_init() {
    let source = r#"
            const z
            return z
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Null));
}

/* ===================== Multiple Declarations ===================== */

#[test]
fn test_multiple_declarations() {
    let source = r#"
            let a = 1
            let b = 2
            let c = 3
            return a + b + c
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(6.0)));
}

#[test]
fn test_mixed_let_const() {
    let source = r#"
            let x = 10
            const y = 20
            let z = 30
            return x + y + z
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(60.0)));
}

/* ===================== Scope Tests ===================== */

#[test]
fn test_block_scope_cleanup() {
    let source = r#"
            {
                let scoped = 999
            }
            return scoped
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // Variable 'scoped' should not be defined outside the block
    match vm.control {
        Control::Throw(Val::Error(ref err)) => {
            assert!(err.message.contains("Undefined variable"));
        }
        _ => panic!(
            "Expected error for undefined variable, got {:?}",
            vm.control
        ),
    }
}

#[test]
fn test_nested_blocks_with_declarations() {
    let source = r#"
            let outer = 1
            {
                let inner = 2
                {
                    let innermost = 3
                    return outer + inner + innermost
                }
            }
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(6.0)));

    // After return, all declared variables should be cleaned up during unwinding
    assert!(!vm.env.contains_key("outer"));
    assert!(!vm.env.contains_key("inner"));
    assert!(!vm.env.contains_key("innermost"));
}

#[test]
fn test_variable_accessible_in_nested_block() {
    let source = r#"
            let x = 42
            {
                {
                    return x
                }
            }
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

/* ===================== Initialization Expression Tests ===================== */

#[test]
fn test_let_with_expression() {
    let source = r#"
            let result = 10 + 32
            return result
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_let_with_member_access() {
    let source = r#"
            let value = Inputs.data
            return value
        "#;

    let inputs = hashmap! {
        "data".to_string() => Val::Num(123.0),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(123.0)));
}

#[test]
fn test_let_with_object_literal() {
    let source = r#"
            let obj = {name: "Alice", age: 30}
            return obj.name
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("Alice".to_string())));
}

#[test]
fn test_let_with_array_literal() {
    let source = r#"
            let arr = [1, 2, 3]
            return arr
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::List(vec![Val::Num(1.0), Val::Num(2.0), Val::Num(3.0)]))
    );
}

/* ===================== Sequential Declarations ===================== */

#[test]
fn test_declare_and_assign() {
    let source = r#"
            let x = 10
            x = 20
            return x
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(20.0)));
}

#[test]
fn test_use_previous_declaration() {
    let source = r#"
            let a = 5
            let b = a + 10
            return b
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(15.0)));
}

/* ===================== Suspension and Scope Cleanup Tests ===================== */

#[test]
fn test_block_scope_cleanup_with_suspension() {
    let source = r#"
            let outer = 10
            {
                let scoped = 100
                let task = await Task.run("test", {})
            }
            return outer
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // Should suspend on the await
    let Control::Suspend(_task_id) = &vm.control else {
        panic!("Expected Suspend, got {:?}", vm.control);
    };

    // While suspended, verify both outer and scoped variables are still in scope
    // (because the block hasn't completed yet - we're suspended inside it)
    assert!(vm.env.contains_key("outer"));
    assert!(vm.env.contains_key("scoped"));

    // Resume execution with a task result
    vm.resume(Val::Str("task_result".to_string()));
    run_until_done(&mut vm);

    // After resumption and block completion, verify scoped variable was cleaned up
    assert_eq!(vm.control, Control::Return(Val::Num(10.0)));

    // After return, all declared variables should be cleaned up during unwinding
    assert!(!vm.env.contains_key("scoped"));
    assert!(!vm.env.contains_key("task"));
    assert!(!vm.env.contains_key("outer"));
}

/* ===================== Control Flow Unwinding Tests ===================== */

#[test]
fn test_throw_unwinds_and_cleans_up_scopes() {
    let source = r#"
            let outer = 1
            try {
                let inTry = 2
                {
                    let nested = 3
                    let bad = Context.missing.property
                }
            } catch (e) {
                return 99
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(99.0)));

    // After unwinding to catch, nested block variables should be cleaned up
    // but outer and inTry remain (catch has access to try block scope)
    assert!(!vm.env.contains_key("nested"));
    assert!(!vm.env.contains_key("bad"));
}

#[test]
fn test_break_unwinds_and_cleans_up_scopes() {
    let source = r#"
            let outer = 1
            while (true) {
                let inLoop = 2
                {
                    let nested = 3
                    break
                }
            }
            return outer
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(1.0)));

    // After break unwinding, nested and loop variables should be cleaned up
    assert!(!vm.env.contains_key("nested"));
    assert!(!vm.env.contains_key("inLoop"));
    // After return unwinding, outer should also be cleaned up
    assert!(!vm.env.contains_key("outer"));
}

#[test]
fn test_continue_unwinds_and_cleans_up_scopes() {
    let source = r#"
            let outer = 1
            let count = 0
            while (count < 2) {
                count = count + 1
                let inLoop = 10
                {
                    let nested = 20
                    continue
                }
            }
            return outer + count
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));

    // After continue unwinding and loop completion, all variables cleaned up
    assert!(!vm.env.contains_key("nested"));
    assert!(!vm.env.contains_key("inLoop"));
    assert!(!vm.env.contains_key("count"));
    assert!(!vm.env.contains_key("outer"));
}

#[test]
fn test_nested_try_blocks_with_cleanup() {
    let source = r#"
            let outer = 1
            try {
                let try1 = 2
                try {
                    let try2 = 3
                    let bad = Context.missing.value
                } catch (inner) {
                    let catchInner = 4
                    return try1 + try2
                }
            } catch (outer_err) {
                return 99
            }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // Inner catch should handle the error, but try2 is not in scope there
    // So this will throw trying to access try2, and outer catch will handle it
    assert_eq!(vm.control, Control::Return(Val::Num(99.0)));

    // All variables should be cleaned up after unwinding and return
    assert!(!vm.env.contains_key("outer"));
    assert!(!vm.env.contains_key("try1"));
    assert!(!vm.env.contains_key("try2"));
}

/* ===================== Error Cases ===================== */

#[test]
fn test_declare_with_throwing_init() {
    let source = r#"
            let x = Inputs.missing.value
            return x
        "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // Should throw an error during initialization
    assert!(matches!(vm.control, Control::Throw(_)));
}

/* ===================== New Syntax Tests ===================== */

#[test]
fn test_new_syntax_simple() {
    // Test the new top-level syntax without async function wrapper
    let source = r#"
        let x = 42
        return x
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_new_syntax_with_front_matter() {
    // Test the new syntax with YAML front matter
    let source = r#"
```
name: test_workflow
description: A test workflow
```
let x = 100
return x
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(100.0)));
}

/* ===================== Destructuring Tests ===================== */

#[test]
fn test_destructure_simple() {
    let source = r#"
        let obj = { a: 1, b: 2 }
        let { a, b } = obj
        return a + b
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}

#[test]
fn test_destructure_from_inputs() {
    let source = r#"
        let { name, age } = Inputs
        return name
    "#;

    let inputs = hashmap! {
        "name".to_string() => Val::Str("Alice".to_string()),
        "age".to_string() => Val::Num(30.0),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("Alice".to_string())));
}

#[test]
fn test_destructure_const() {
    let source = r#"
        const { x, y } = { x: 10, y: 20 }
        return x + y
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(30.0)));
}

#[test]
fn test_destructure_missing_property_throws() {
    // Missing property should throw PROPERTY_NOT_FOUND
    let source = r#"
        let { a, missing } = { a: 42 }
        return missing
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    match vm.control {
        Control::Throw(Val::Error(ref err)) => {
            assert_eq!(err.code, "PROPERTY_NOT_FOUND");
            assert!(err.message.contains("missing"));
        }
        _ => panic!("Expected PROPERTY_NOT_FOUND error, got {:?}", vm.control),
    }
}

#[test]
fn test_destructure_single_property() {
    let source = r#"
        let { id } = { id: 123, extra: "ignored" }
        return id
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(123.0)));
}

#[test]
fn test_destructure_non_object_throws() {
    let source = r#"
        let { a } = 42
        return a
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    match vm.control {
        Control::Throw(Val::Error(ref err)) => {
            assert!(err.message.contains("Cannot destructure non-object"));
        }
        _ => panic!("Expected TypeError, got {:?}", vm.control),
    }
}

#[test]
fn test_destructure_scope_cleanup() {
    let source = r#"
        {
            let { a, b } = { a: 1, b: 2 }
        }
        return a
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    // 'a' should not be defined outside the block
    match vm.control {
        Control::Throw(Val::Error(ref err)) => {
            assert!(err.message.contains("Undefined variable"));
        }
        _ => panic!(
            "Expected error for undefined variable, got {:?}",
            vm.control
        ),
    }
}

#[test]
fn test_destructure_trailing_comma() {
    let source = r#"
        let { a, b, } = { a: 1, b: 2 }
        return a + b
    "#;

    let mut vm = parse_workflow_and_build_vm(source, hashmap! {});
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(3.0)));
}
