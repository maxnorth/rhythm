//! Tests for assignment statements

use super::helpers::parse_workflow_and_build_vm;
use crate::executor::{run_until_done, Control, Val};
use maplit::hashmap;
use std::collections::HashMap;

/* ===================== Basic Assignment Tests ===================== */

#[test]
fn test_assign_number() {
    // x = 42; return x;
    let source = r#"
            x = 42
            return x
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
    assert_eq!(vm.env.get("x"), Some(&Val::Num(42.0)));
}

#[test]
fn test_assign_string() {
    // name = "Alice"; return name;
    let source = r#"
            name = "Alice"
            return name
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("Alice".to_string())));
    assert_eq!(vm.env.get("name"), Some(&Val::Str("Alice".to_string())));
}

#[test]
fn test_assign_array() {
    // items = [1, 2, 3]; return items;
    let source = r#"
            items = [1, 2, 3]
            return items
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let expected = Val::List(vec![Val::Num(1.0), Val::Num(2.0), Val::Num(3.0)]);
    assert_eq!(vm.control, Control::Return(expected.clone()));
    assert_eq!(vm.env.get("items"), Some(&expected));
}

#[test]
fn test_assign_object() {
    // user = {name: "Bob", age: 30}; return user;
    let source = r#"
            user = {name: "Bob", age: 30}
            return user
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    let expected = Val::Obj(hashmap! {
        "name".to_string() => Val::Str("Bob".to_string()),
        "age".to_string() => Val::Num(30.0),
    });

    assert_eq!(vm.control, Control::Return(expected.clone()));
    assert_eq!(vm.env.get("user"), Some(&expected));
}

/* ===================== Assignment with Expressions ===================== */

#[test]
fn test_assign_from_variable() {
    // y = Inputs.x; return y;
    let source = r#"
            y = Inputs.x
            return y
        "#;

    let env = hashmap! {
        "x".to_string() => Val::Num(100.0),
    };

    let mut vm = parse_workflow_and_build_vm(source, env);
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(100.0)));
    assert_eq!(vm.env.get("y"), Some(&Val::Num(100.0)));
}

#[test]
fn test_assign_with_member_access() {
    // name = Context.user; return name;
    let source = r#"
            name = Context.user
            return name
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());

    // Set up Context with user property
    let ctx_obj = hashmap! {
        "user".to_string() => Val::Str("Alice".to_string()),
    };
    vm.env.insert("Context".to_string(), Val::Obj(ctx_obj));

    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("Alice".to_string())));
    assert_eq!(vm.env.get("name"), Some(&Val::Str("Alice".to_string())));
}

#[test]
fn test_assign_with_function_call() {
    // result = Math.abs(Inputs.x); return result;
    let source = r#"
            result = Math.abs(Inputs.x)
            return result
        "#;

    let env = hashmap! {
        "x".to_string() => Val::Num(-42.0),
    };

    let mut vm = parse_workflow_and_build_vm(source, env);
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
    assert_eq!(vm.env.get("result"), Some(&Val::Num(42.0)));
}

/* ===================== Reassignment Tests ===================== */

#[test]
fn test_reassignment() {
    // x = 10; x = 20; return x;
    let source = r#"
            x = 10
            x = 20
            return x
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(20.0)));
    assert_eq!(vm.env.get("x"), Some(&Val::Num(20.0)));
}

#[test]
fn test_reassignment_different_type() {
    // x = 42; x = "hello"; return x;
    let source = r#"
            x = 42
            x = "hello"
            return x
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("hello".to_string())));
    assert_eq!(vm.env.get("x"), Some(&Val::Str("hello".to_string())));
}

/* ===================== Assignment with Await ===================== */

#[test]
fn test_assign_with_await() {
    // result = await Task.run("my_task", {});
    let source = r#"
            result = await Task.run("my_task", {})
            return result
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should be suspended on the awaited task
    match &vm.control {
        Control::Suspend(crate::executor::Awaitable::Task(task_id)) => {
            assert_eq!(task_id.len(), 36); // UUID format
        }
        _ => panic!(
            "Expected Control::Suspend(Awaitable::Task(_)), got {:?}",
            vm.control
        ),
    }

    // The assignment should NOT have completed yet (variable not in env)
    assert_eq!(vm.env.get("result"), None);

    // Frames should be preserved (not popped due to suspension)
    assert_eq!(vm.frames.len(), 2); // Block + Assign frames
}

#[test]
fn test_assign_with_await_resume() {
    // result = await Task.run("my_task", {}); return result;
    let source = r#"
            result = await Task.run("my_task", {})
            return result
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Verify suspension
    assert!(matches!(vm.control, Control::Suspend(_)));

    // Resume with a result value
    let task_result = Val::Num(42.0);
    assert!(vm.resume(task_result.clone()));

    // Run to completion
    run_until_done(&mut vm);

    // Should return the result
    assert_eq!(vm.control, Control::Return(Val::Num(42.0)));
    // The assignment should have completed
    assert_eq!(vm.env.get("result"), Some(&Val::Num(42.0)));
}

/* ===================== Assignment with Error Handling ===================== */

#[test]
fn test_assign_with_error() {
    // result = Context.nonexistent;
    let source = r#"
            result = Context.nonexistent
            return result
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw an error
    match &vm.control {
        Control::Throw(Val::Error(err)) => {
            assert!(err.message.contains("nonexistent"));
        }
        _ => panic!("Expected Control::Throw, got {:?}", vm.control),
    }

    // The assignment should NOT have completed
    assert_eq!(vm.env.get("result"), None);
}

#[test]
fn test_assign_in_try_catch() {
    // try { result = Context.bad; } catch (e) { result = "error"; } return result;
    let source = r#"
            try {
                result = Context.bad
            } catch (e) {
                result = "error"
            }
            return result
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return "error" from the catch block
    assert_eq!(vm.control, Control::Return(Val::Str("error".to_string())));
    assert_eq!(vm.env.get("result"), Some(&Val::Str("error".to_string())));
}

/* ===================== Multiple Assignments ===================== */

#[test]
fn test_multiple_assignments() {
    // a = 1; b = 2; c = 3; return [a, b, c];
    let source = r#"
            a = 1
            b = 2
            c = 3
            return [a, b, c]
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(
        vm.control,
        Control::Return(Val::List(vec![Val::Num(1.0), Val::Num(2.0), Val::Num(3.0)]))
    );
    assert_eq!(vm.env.get("a"), Some(&Val::Num(1.0)));
    assert_eq!(vm.env.get("b"), Some(&Val::Num(2.0)));
    assert_eq!(vm.env.get("c"), Some(&Val::Num(3.0)));
}

/* ===================== Attribute Assignment Tests ===================== */

#[test]
fn test_assign_object_property() {
    // user = {name: "Alice", age: 25}; user.name = "Bob"; return user.name;
    let source = r#"
            user = {name: "Alice", age: 25}
            user.name = "Bob"
            return user.name
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Str("Bob".to_string())));

    // Verify the object was mutated
    if let Some(Val::Obj(user)) = vm.env.get("user") {
        assert_eq!(user.get("name"), Some(&Val::Str("Bob".to_string())));
        assert_eq!(user.get("age"), Some(&Val::Num(25.0)));
    } else {
        panic!("Expected user to be an object");
    }
}

#[test]
fn test_assign_array_index() {
    // items = [1, 2, 3]; items[1] = 99; return items;
    let source = r#"
            items = [1, 2, 3]
            items[1] = 99
            return items
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Verify the array was mutated
    let expected = Val::List(vec![Val::Num(1.0), Val::Num(99.0), Val::Num(3.0)]);
    assert_eq!(vm.control, Control::Return(expected.clone()));
    assert_eq!(vm.env.get("items"), Some(&expected));
}

#[test]
fn test_assign_nested_property() {
    // config = {db: {host: "localhost", port: 5432}}; config.db.port = 5433; return config.db.port;
    let source = r#"
            config = {db: {host: "localhost", port: 5432}}
            config.db.port = 5433
            return config.db.port
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    assert_eq!(vm.control, Control::Return(Val::Num(5433.0)));
}

#[test]
fn test_assign_mixed_path() {
    // data = {items: [1, 2, 3]}; data.items[0] = 99; return data;
    let source = r#"
            data = {items: [1, 2, 3]}
            data.items[0] = 99
            return data
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Verify the nested property was mutated
    if let Some(Val::Obj(data)) = vm.env.get("data") {
        if let Some(Val::List(items)) = data.get("items") {
            assert_eq!(items, &vec![Val::Num(99.0), Val::Num(2.0), Val::Num(3.0)]);
        } else {
            panic!("Expected data.items to be a list");
        }
    } else {
        panic!("Expected data to be an object");
    }
}

#[test]
fn test_assign_computed_index() {
    // arr = [10, 20, 30]; i = 1; arr[i] = 99; return arr;
    let source = r#"
            arr = [10, 20, 30]
            i = 1
            arr[i] = 99
            return arr
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Verify the array was mutated at the computed index
    let expected = Val::List(vec![Val::Num(10.0), Val::Num(99.0), Val::Num(30.0)]);
    assert_eq!(vm.control, Control::Return(expected.clone()));
    assert_eq!(vm.env.get("arr"), Some(&expected));
}

#[test]
fn test_assign_prop_access_on_non_object_error() {
    // x = 42; x.foo = "bar"; (should error - can't use Prop on number)
    let source = r#"
            x = 42
            x.foo = "bar"
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should get a TypeError for trying to use Prop access on a number
    match &vm.control {
        Control::Throw(Val::Error(err)) => {
            assert_eq!(err.code, "TypeError");
            assert!(err
                .message
                .contains("Cannot set property 'foo' on non-object value"));
        }
        _ => panic!("Expected TypeError, got: {:?}", vm.control),
    }
}

#[test]
fn test_assign_index_access_on_primitive_error() {
    // x = 42; x[0] = "bar"; (should error - can't use Index on number)
    let source = r#"
            x = 42
            x[0] = "bar"
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should get a TypeError for trying to use Index access on a number
    match &vm.control {
        Control::Throw(Val::Error(err)) => {
            assert_eq!(err.code, "TypeError");
            assert!(err
                .message
                .contains("Cannot use index access on non-object/non-array value"));
        }
        _ => panic!("Expected TypeError, got: {:?}", vm.control),
    }
}

#[test]
fn test_assign_prop_access_on_array_error() {
    // arr = [1, 2, 3]; arr.foo = "bar"; (should error - can't use Prop on array)
    let source = r#"
            arr = [1, 2, 3]
            arr.foo = "bar"
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should get a TypeError for trying to use Prop access on an array
    match &vm.control {
        Control::Throw(Val::Error(err)) => {
            assert_eq!(err.code, "TypeError");
            assert!(err
                .message
                .contains("Cannot set property 'foo' on non-object value"));
        }
        _ => panic!("Expected TypeError, got: {:?}", vm.control),
    }
}

#[test]
fn test_assign_nested_prop_access_on_non_object_error() {
    // obj = {a: 42}; obj.a.b = "bar"; (should error - can't use Prop on number)
    let source = r#"
            obj = {a: 42}
            obj.a.b = "bar"
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should get a TypeError when trying to set .b on the number 42
    match &vm.control {
        Control::Throw(Val::Error(err)) => {
            assert_eq!(err.code, "TypeError");
            assert!(err
                .message
                .contains("Cannot set property 'b' on non-object value"));
        }
        _ => panic!("Expected TypeError, got: {:?}", vm.control),
    }
}

#[test]
fn test_assign_index_access_on_object_allowed() {
    // obj = {}; obj["foo"] = "bar"; return obj; (should work - Index allowed on objects)
    let source = r#"
            obj = {}
            obj["foo"] = "bar"
            return obj
        "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should succeed - Index access is allowed on objects
    let expected = Val::Obj(hashmap! {
        "foo".to_string() => Val::Str("bar".to_string()),
    });
    assert_eq!(vm.control, Control::Return(expected.clone()));
    assert_eq!(vm.env.get("obj"), Some(&expected));
}
