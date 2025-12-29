//! Tests for Promise.all(), Promise.any(), Promise.race() composite awaitables

use super::helpers::parse_workflow_and_build_vm;
use crate::executor::{errors, run_until_done, Awaitable, Control, Val};
use std::collections::HashMap;

/* ===================== Promise.all() Tests ===================== */

#[test]
fn test_task_all_with_array() {
    let source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        return Promise.all([t1, t2])
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise(All) with two items
    match &vm.control {
        Control::Return(Val::Promise(Awaitable::All { items, is_object })) => {
            assert_eq!(items.len(), 2);
            assert!(!*is_object);
            // Keys should be "0" and "1" for array form
            assert_eq!(items[0].0, "0");
            assert_eq!(items[1].0, "1");
            // Both should be Task awaitables
            assert!(matches!(items[0].1, Awaitable::Task(_)));
            assert!(matches!(items[1].1, Awaitable::Task(_)));
        }
        _ => panic!(
            "Expected Control::Return(Val::Promise(Awaitable::All)), got {:?}",
            vm.control
        ),
    }
}

#[test]
fn test_task_all_with_object() {
    let source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        return Promise.all({ first: t1, second: t2 })
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise(All) with is_object=true
    match &vm.control {
        Control::Return(Val::Promise(Awaitable::All { items, is_object })) => {
            assert_eq!(items.len(), 2);
            assert!(*is_object);
            // Keys should be sorted alphabetically for determinism
            assert_eq!(items[0].0, "first");
            assert_eq!(items[1].0, "second");
        }
        _ => panic!(
            "Expected Control::Return(Val::Promise(Awaitable::All)), got {:?}",
            vm.control
        ),
    }
}

#[test]
fn test_task_all_empty_array_returns_immediately() {
    let source = r#"
        return Promise.all([])
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Empty array should return empty array immediately (no promise)
    match &vm.control {
        Control::Return(Val::List(list)) => {
            assert!(list.is_empty());
        }
        _ => panic!(
            "Expected Control::Return(Val::List([])), got {:?}",
            vm.control
        ),
    }
}

#[test]
fn test_task_all_empty_object_returns_immediately() {
    let source = r#"
        return Promise.all({})
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Empty object should return empty object immediately (no promise)
    match &vm.control {
        Control::Return(Val::Obj(obj)) => {
            assert!(obj.is_empty());
        }
        _ => panic!(
            "Expected Control::Return(Val::Obj({{}})), got {:?}",
            vm.control
        ),
    }
}

#[test]
fn test_task_all_non_promise_throws() {
    let source = r#"
        return Promise.all([42])
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw error because 42 is not a promise
    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_TYPE);
    assert!(err.message.contains("Promise"));
}

#[test]
fn test_task_all_wrong_arg_type() {
    let source = r#"
        return Promise.all(42)
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should throw error because argument must be array or object
    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_TYPE);
}

/* ===================== Promise.any() Tests ===================== */

#[test]
fn test_task_any_with_array() {
    let source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        return Promise.any([t1, t2])
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise(Any) with two items
    match &vm.control {
        Control::Return(Val::Promise(Awaitable::Any {
            items,
            is_object,
            with_kv,
        })) => {
            assert_eq!(items.len(), 2);
            assert!(!*is_object);
            assert!(!*with_kv); // Promise.any returns just the value
            assert_eq!(items[0].0, "0");
            assert_eq!(items[1].0, "1");
        }
        _ => panic!(
            "Expected Control::Return(Val::Promise(Awaitable::Any)), got {:?}",
            vm.control
        ),
    }
}

#[test]
fn test_task_any_with_object() {
    let source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        return Promise.any({ alpha: t1, beta: t2 })
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise(Any) with is_object=true
    match &vm.control {
        Control::Return(Val::Promise(Awaitable::Any {
            items,
            is_object,
            with_kv,
        })) => {
            assert_eq!(items.len(), 2);
            assert!(*is_object);
            assert!(!*with_kv); // Promise.any returns just the value
                                // Keys sorted alphabetically
            assert_eq!(items[0].0, "alpha");
            assert_eq!(items[1].0, "beta");
        }
        _ => panic!(
            "Expected Control::Return(Val::Promise(Awaitable::Any)), got {:?}",
            vm.control
        ),
    }
}

#[test]
fn test_task_any_empty_throws_aggregate_error() {
    let source = r#"
        return Promise.any([])
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Empty input should throw AggregateError
    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, "AggregateError");
}

/* ===================== Promise.race() Tests ===================== */

#[test]
fn test_task_race_with_array() {
    let source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        return Promise.race([t1, t2])
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise(Race) with two items
    match &vm.control {
        Control::Return(Val::Promise(Awaitable::Race {
            items,
            is_object,
            with_kv,
        })) => {
            assert_eq!(items.len(), 2);
            assert!(!*is_object);
            assert!(!*with_kv); // Promise.race returns just the value
            assert_eq!(items[0].0, "0");
            assert_eq!(items[1].0, "1");
        }
        _ => panic!(
            "Expected Control::Return(Val::Promise(Awaitable::Race)), got {:?}",
            vm.control
        ),
    }
}

#[test]
fn test_task_race_with_object() {
    let source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        return Promise.race({ fast: t1, slow: t2 })
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise(Race) with is_object=true
    match &vm.control {
        Control::Return(Val::Promise(Awaitable::Race {
            items,
            is_object,
            with_kv,
        })) => {
            assert_eq!(items.len(), 2);
            assert!(*is_object);
            assert!(!*with_kv); // Promise.race returns just the value
                                // Keys sorted alphabetically
            assert_eq!(items[0].0, "fast");
            assert_eq!(items[1].0, "slow");
        }
        _ => panic!(
            "Expected Control::Return(Val::Promise(Awaitable::Race)), got {:?}",
            vm.control
        ),
    }
}

#[test]
fn test_task_race_empty_throws() {
    let source = r#"
        return Promise.race([])
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Empty input should throw error
    let Control::Throw(Val::Error(err)) = vm.control else {
        panic!("Expected Control::Throw with Error, got {:?}", vm.control);
    };
    assert_eq!(err.code, errors::WRONG_ARG_TYPE);
    assert!(err.message.contains("at least one"));
}

/* ===================== Await Tests for Composites ===================== */

#[test]
fn test_await_task_all_suspends() {
    let source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        return await Promise.all([t1, t2])
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should suspend on the All awaitable
    match &vm.control {
        Control::Suspend(Awaitable::All { items, is_object }) => {
            assert_eq!(items.len(), 2);
            assert!(!*is_object);
        }
        _ => panic!(
            "Expected Control::Suspend(Awaitable::All), got {:?}",
            vm.control
        ),
    }

    // Outbox should have 2 tasks
    assert_eq!(vm.outbox.tasks.len(), 2);
}

#[test]
fn test_await_task_any_suspends() {
    let source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        return await Promise.any([t1, t2])
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should suspend on the Any awaitable
    match &vm.control {
        Control::Suspend(Awaitable::Any {
            items, is_object, ..
        }) => {
            assert_eq!(items.len(), 2);
            assert!(!*is_object);
        }
        _ => panic!(
            "Expected Control::Suspend(Awaitable::Any), got {:?}",
            vm.control
        ),
    }
}

#[test]
fn test_await_task_race_suspends() {
    let source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        return await Promise.race([t1, t2])
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should suspend on the Race awaitable
    match &vm.control {
        Control::Suspend(Awaitable::Race {
            items, is_object, ..
        }) => {
            assert_eq!(items.len(), 2);
            assert!(!*is_object);
        }
        _ => panic!(
            "Expected Control::Suspend(Awaitable::Race), got {:?}",
            vm.control
        ),
    }
}

/* ===================== Mixed Composites Tests ===================== */

#[test]
fn test_task_all_with_timers() {
    let source = r#"
        let timer1 = Timer.delay(1)
        let timer2 = Timer.delay(2)
        return Promise.all([timer1, timer2])
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise(All) with timer awaitables
    match &vm.control {
        Control::Return(Val::Promise(Awaitable::All { items, is_object })) => {
            assert_eq!(items.len(), 2);
            assert!(!*is_object);
            // Both should be Timer awaitables
            assert!(matches!(items[0].1, Awaitable::Timer { .. }));
            assert!(matches!(items[1].1, Awaitable::Timer { .. }));
        }
        _ => panic!(
            "Expected Control::Return(Val::Promise(Awaitable::All)), got {:?}",
            vm.control
        ),
    }
}

#[test]
fn test_task_race_mixed_task_and_timer() {
    let source = r#"
        let task = Task.run("slow_task", {})
        let timer = Timer.delay(5)
        return Promise.race([task, timer])
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise(Race) with mixed awaitables
    match &vm.control {
        Control::Return(Val::Promise(Awaitable::Race {
            items, is_object, ..
        })) => {
            assert_eq!(items.len(), 2);
            assert!(!*is_object);
            // First should be Task, second should be Timer
            assert!(matches!(items[0].1, Awaitable::Task(_)));
            assert!(matches!(items[1].1, Awaitable::Timer { .. }));
        }
        _ => panic!(
            "Expected Control::Return(Val::Promise(Awaitable::Race)), got {:?}",
            vm.control
        ),
    }
}

/* ===================== Promise.any_kv() Tests ===================== */

#[test]
fn test_task_any_kv_with_array() {
    let source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        return Promise.any_kv([t1, t2])
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise(Any) with with_kv=true
    match &vm.control {
        Control::Return(Val::Promise(Awaitable::Any {
            items,
            is_object,
            with_kv,
        })) => {
            assert_eq!(items.len(), 2);
            assert!(!*is_object);
            assert!(*with_kv); // any_kv returns { key, value }
        }
        _ => panic!(
            "Expected Control::Return(Val::Promise(Awaitable::Any)), got {:?}",
            vm.control
        ),
    }
}

#[test]
fn test_task_any_kv_with_object() {
    let source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        return Promise.any_kv({ alpha: t1, beta: t2 })
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise(Any) with with_kv=true
    match &vm.control {
        Control::Return(Val::Promise(Awaitable::Any {
            items,
            is_object,
            with_kv,
        })) => {
            assert_eq!(items.len(), 2);
            assert!(*is_object);
            assert!(*with_kv); // any_kv returns { key, value }
        }
        _ => panic!(
            "Expected Control::Return(Val::Promise(Awaitable::Any)), got {:?}",
            vm.control
        ),
    }
}

/* ===================== Promise.race_kv() Tests ===================== */

#[test]
fn test_task_race_kv_with_array() {
    let source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        return Promise.race_kv([t1, t2])
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise(Race) with with_kv=true
    match &vm.control {
        Control::Return(Val::Promise(Awaitable::Race {
            items,
            is_object,
            with_kv,
        })) => {
            assert_eq!(items.len(), 2);
            assert!(!*is_object);
            assert!(*with_kv); // race_kv returns { key, value }
        }
        _ => panic!(
            "Expected Control::Return(Val::Promise(Awaitable::Race)), got {:?}",
            vm.control
        ),
    }
}

#[test]
fn test_task_race_kv_with_object() {
    let source = r#"
        let t1 = Task.run("task1", {})
        let t2 = Task.run("task2", {})
        return Promise.race_kv({ fast: t1, slow: t2 })
    "#;

    let mut vm = parse_workflow_and_build_vm(source, HashMap::new());
    run_until_done(&mut vm);

    // Should return a Promise(Race) with with_kv=true
    match &vm.control {
        Control::Return(Val::Promise(Awaitable::Race {
            items,
            is_object,
            with_kv,
        })) => {
            assert_eq!(items.len(), 2);
            assert!(*is_object);
            assert!(*with_kv); // race_kv returns { key, value }
        }
        _ => panic!(
            "Expected Control::Return(Val::Promise(Awaitable::Race)), got {:?}",
            vm.control
        ),
    }
}
