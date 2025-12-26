//! Tests for literal expressions (arrays and objects)

use super::helpers::parse_workflow_and_build_vm;
use crate::executor::{run_until_done, Control, Val};
use std::collections::HashMap;

/* ===================== Array Literal Tests ===================== */

#[test]
fn test_array_literal_empty() {
    let source = r#"
            return []
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::List(vec![])));
}

#[test]
fn test_array_literal_numbers() {
    let source = r#"
            return [1, 2, 3]
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::List(vec![Val::Num(1.0), Val::Num(2.0), Val::Num(3.0)]))
    );
}

#[test]
fn test_array_literal_mixed_types() {
    let source = r#"
            return [42, "hello", true]
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::List(vec![
            Val::Num(42.0),
            Val::Str("hello".to_string()),
            Val::Bool(true)
        ]))
    );
}

#[test]
fn test_array_literal_nested() {
    let source = r#"
            return [[1, 2], [3, 4]]
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::List(vec![
            Val::List(vec![Val::Num(1.0), Val::Num(2.0)]),
            Val::List(vec![Val::Num(3.0), Val::Num(4.0)])
        ]))
    );
}

#[test]
fn test_array_literal_with_expressions() {
    // [Inputs.x, Inputs.y] where x=10, y=20
    let source = r#"
            return [Inputs.x, Inputs.y]
        "#;

    let mut env = HashMap::new();
    env.insert("x".to_string(), Val::Num(10.0));
    env.insert("y".to_string(), Val::Num(20.0));

    let mut vm = parse_workflow_and_build_vm(source, env);
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::List(vec![Val::Num(10.0), Val::Num(20.0)]))
    );
}

/* ===================== Object Literal Tests ===================== */

#[test]
fn test_object_literal_empty() {
    let source = r#"
            return {}
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Obj(HashMap::new())));
}

#[test]
fn test_object_literal_simple() {
    let source = r#"
            return {name: "Alice", age: 30}
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let mut expected = HashMap::new();
    expected.insert("name".to_string(), Val::Str("Alice".to_string()));
    expected.insert("age".to_string(), Val::Num(30.0));

    assert_eq!(vm.control, Control::Return(Val::Obj(expected)));
}

#[test]
fn test_object_literal_nested() {
    let source = r#"
            return {user: {name: "Bob", id: 123}}
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let mut inner = HashMap::new();
    inner.insert("name".to_string(), Val::Str("Bob".to_string()));
    inner.insert("id".to_string(), Val::Num(123.0));

    let mut outer = HashMap::new();
    outer.insert("user".to_string(), Val::Obj(inner));

    assert_eq!(vm.control, Control::Return(Val::Obj(outer)));
}

#[test]
fn test_object_literal_with_expressions() {
    // {x: Inputs.a, y: Inputs.b} where a=10, b=20
    let source = r#"
            return {x: Inputs.a, y: Inputs.b}
        "#;

    let mut env = HashMap::new();
    env.insert("a".to_string(), Val::Num(10.0));
    env.insert("b".to_string(), Val::Num(20.0));

    let mut vm = parse_workflow_and_build_vm(source, env);
    run_until_done(&mut vm);

    let mut expected = HashMap::new();
    expected.insert("x".to_string(), Val::Num(10.0));
    expected.insert("y".to_string(), Val::Num(20.0));

    assert_eq!(vm.control, Control::Return(Val::Obj(expected)));
}

#[test]
fn test_object_literal_with_array() {
    let source = r#"
            return {items: [1, 2, 3]}
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let mut expected = HashMap::new();
    expected.insert(
        "items".to_string(),
        Val::List(vec![Val::Num(1.0), Val::Num(2.0), Val::Num(3.0)]),
    );

    assert_eq!(vm.control, Control::Return(Val::Obj(expected)));
}

/* ===================== Combined Tests ===================== */

#[test]
fn test_array_in_task_run() {
    // Task.run("my_task", {items: [1, 2, 3]})
    let source = r#"
            return Task.run("my_task", {items: [1, 2, 3]})
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise(Task) value
    assert!(matches!(
        vm.control,
        Control::Return(Val::Promise(crate::executor::Awaitable::Task(_)))
    ));

    // Check outbox has the task with array in inputs
    assert_eq!(vm.outbox.tasks.len(), 1);
    assert_eq!(vm.outbox.tasks[0].task_name, "my_task");

    let items = vm.outbox.tasks[0].inputs.get("items").unwrap();
    assert_eq!(
        items,
        &Val::List(vec![Val::Num(1.0), Val::Num(2.0), Val::Num(3.0)])
    );
}

/* ===================== Multiline Literal Tests ===================== */

#[test]
fn test_multiline_object_literal() {
    // Test object literal with properties on multiple lines
    let source = r#"
        return {
            name: "Alice",
            age: 30,
            city: "New York"
        }
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let expected = maplit::hashmap! {
        "name".to_string() => Val::Str("Alice".to_string()),
        "age".to_string() => Val::Num(30.0),
        "city".to_string() => Val::Str("New York".to_string()),
    };

    assert_eq!(vm.control, Control::Return(Val::Obj(expected)));
}

#[test]
fn test_multiline_function_call() {
    // Test function call with arguments on multiple lines
    let source = r#"
        return add(
            10,
            32
        )
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
}

#[test]
fn test_task_run_with_multiline_object() {
    // Test Task.run with multiline object argument
    let source = r#"
        return Task.run("processOrder", {
            orderId: 123,
            userId: 456,
            total: 99.99
        })
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise(Task) value
    assert!(matches!(
        vm.control,
        Control::Return(Val::Promise(crate::executor::Awaitable::Task(_)))
    ));

    // Check outbox has the task with correct inputs
    assert_eq!(vm.outbox.tasks.len(), 1);
    assert_eq!(vm.outbox.tasks[0].task_name, "processOrder");

    let inputs = &vm.outbox.tasks[0].inputs;
    assert_eq!(inputs.get("orderId").unwrap(), &Val::Num(123.0));
    assert_eq!(inputs.get("userId").unwrap(), &Val::Num(456.0));
    assert_eq!(inputs.get("total").unwrap(), &Val::Num(99.99));
}

/* ===================== Object Shorthand Tests ===================== */

#[test]
fn test_object_shorthand_simple() {
    // Test ES6-style shorthand: { a } means { a: a }
    let source = r#"
        let a = 9
        let b = { a }
        return b
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let expected = maplit::hashmap! {
        "a".to_string() => Val::Num(9.0),
    };

    assert_eq!(vm.control, Control::Return(Val::Obj(expected)));
}

#[test]
fn test_object_shorthand_multiple() {
    // Test multiple shorthand properties
    let source = r#"
        let name = "Alice"
        let age = 30
        let result = { name, age }
        return result
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let expected = maplit::hashmap! {
        "name".to_string() => Val::Str("Alice".to_string()),
        "age".to_string() => Val::Num(30.0),
    };

    assert_eq!(vm.control, Control::Return(Val::Obj(expected)));
}

#[test]
fn test_object_shorthand_mixed() {
    // Test mixing shorthand and regular properties
    let source = r#"
        let x = 10
        let y = 20
        let result = { x, sum: x + y, y }
        return result
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let expected = maplit::hashmap! {
        "x".to_string() => Val::Num(10.0),
        "sum".to_string() => Val::Num(30.0),
        "y".to_string() => Val::Num(20.0),
    };

    assert_eq!(vm.control, Control::Return(Val::Obj(expected)));
}

#[test]
fn test_object_shorthand_from_inputs() {
    // Test shorthand using values from Inputs
    let source = r#"
        let userId = Inputs.userId
        let userName = Inputs.userName
        return { userId, userName }
    "#;

    let inputs = maplit::hashmap! {
        "userId".to_string() => Val::Num(123.0),
        "userName".to_string() => Val::Str("Bob".to_string()),
    };

    let mut vm = parse_workflow_and_build_vm(source, inputs);
    run_until_done(&mut vm);

    let expected = maplit::hashmap! {
        "userId".to_string() => Val::Num(123.0),
        "userName".to_string() => Val::Str("Bob".to_string()),
    };

    assert_eq!(vm.control, Control::Return(Val::Obj(expected)));
}
