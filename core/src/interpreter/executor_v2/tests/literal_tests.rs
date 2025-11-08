//! Tests for literal expressions (arrays and objects)

use crate::interpreter::executor_v2::{run_until_done, Control, Stmt, Val, VM};
use std::collections::HashMap;

/* ===================== Array Literal Tests ===================== */

#[test]
fn test_array_literal_empty() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "LitList",
                "elements": []
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::List(vec![])));
}

#[test]
fn test_array_literal_numbers() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "LitList",
                "elements": [
                    {"t": "LitNum", "v": 1.0},
                    {"t": "LitNum", "v": 2.0},
                    {"t": "LitNum", "v": 3.0}
                ]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::List(vec![Val::Num(1.0), Val::Num(2.0), Val::Num(3.0)]))
    );
}

#[test]
fn test_array_literal_mixed_types() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "LitList",
                "elements": [
                    {"t": "LitNum", "v": 42.0},
                    {"t": "LitStr", "v": "hello"},
                    {"t": "LitBool", "v": true}
                ]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
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
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "LitList",
                "elements": [
                    {
                        "t": "LitList",
                        "elements": [
                            {"t": "LitNum", "v": 1.0},
                            {"t": "LitNum", "v": 2.0}
                        ]
                    },
                    {
                        "t": "LitList",
                        "elements": [
                            {"t": "LitNum", "v": 3.0},
                            {"t": "LitNum", "v": 4.0}
                        ]
                    }
                ]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
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
    // [x, y] where x=10, y=20
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "LitList",
                "elements": [
                    {"t": "Ident", "name": "x"},
                    {"t": "Ident", "name": "y"}
                ]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("x".to_string(), Val::Num(10.0));
    env.insert("y".to_string(), Val::Num(20.0));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::List(vec![Val::Num(10.0), Val::Num(20.0)]))
    );
}

/* ===================== Object Literal Tests ===================== */

#[test]
fn test_object_literal_empty() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "LitObj",
                "properties": []
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Obj(HashMap::new())));
}

#[test]
fn test_object_literal_simple() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "LitObj",
                "properties": [
                    ["name", {"t": "LitStr", "v": "Alice"}],
                    ["age", {"t": "LitNum", "v": 30.0}]
                ]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    let mut expected = HashMap::new();
    expected.insert("name".to_string(), Val::Str("Alice".to_string()));
    expected.insert("age".to_string(), Val::Num(30.0));

    assert_eq!(vm.control, Control::Return(Val::Obj(expected)));
}

#[test]
fn test_object_literal_nested() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "LitObj",
                "properties": [
                    ["user", {
                        "t": "LitObj",
                        "properties": [
                            ["name", {"t": "LitStr", "v": "Bob"}],
                            ["id", {"t": "LitNum", "v": 123.0}]
                        ]
                    }]
                ]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
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
    // {x: a, y: b} where a=10, b=20
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "LitObj",
                "properties": [
                    ["x", {"t": "Ident", "name": "a"}],
                    ["y", {"t": "Ident", "name": "b"}]
                ]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();

    let mut env = HashMap::new();
    env.insert("a".to_string(), Val::Num(10.0));
    env.insert("b".to_string(), Val::Num(20.0));

    let mut vm = VM::new(program, env);
    run_until_done(&mut vm);

    let mut expected = HashMap::new();
    expected.insert("x".to_string(), Val::Num(10.0));
    expected.insert("y".to_string(), Val::Num(20.0));

    assert_eq!(vm.control, Control::Return(Val::Obj(expected)));
}

#[test]
fn test_object_literal_with_array() {
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "LitObj",
                "properties": [
                    ["items", {
                        "t": "LitList",
                        "elements": [
                            {"t": "LitNum", "v": 1.0},
                            {"t": "LitNum", "v": 2.0},
                            {"t": "LitNum", "v": 3.0}
                        ]
                    }]
                ]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
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
    let program_json = r#"{
        "t": "Block",
        "body": [{
            "t": "Return",
            "value": {
                "t": "Call",
                "callee": {
                    "t": "Member",
                    "object": {
                        "t": "Ident",
                        "name": "Task"
                    },
                    "property": "run"
                },
                "args": [{
                    "t": "LitStr",
                    "v": "my_task"
                }, {
                    "t": "LitObj",
                    "properties": [
                        ["items", {
                            "t": "LitList",
                            "elements": [
                                {"t": "LitNum", "v": 1.0},
                                {"t": "LitNum", "v": 2.0},
                                {"t": "LitNum", "v": 3.0}
                            ]
                        }]
                    ]
                }]
            }
        }]
    }"#;

    let program: Stmt = serde_json::from_str(program_json).unwrap();
    let mut vm = VM::new(program, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Task value
    assert!(matches!(vm.control, Control::Return(Val::Task(_))));

    // Check outbox has the task with array in inputs
    assert_eq!(vm.outbox.len(), 1);
    assert_eq!(vm.outbox[0].task_name, "my_task");

    let items = vm.outbox[0].inputs.get("items").unwrap();
    assert_eq!(
        items,
        &Val::List(vec![Val::Num(1.0), Val::Num(2.0), Val::Num(3.0)])
    );
}
