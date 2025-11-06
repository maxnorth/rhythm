/// Tests for expression evaluation
///
/// This module contains comprehensive tests for the evaluate_expression() function,
/// which is a critical part of the workflow execution engine.

#[cfg(test)]
mod tests {
    use serde_json::json;
    use crate::interpreter::executor::{evaluate_expression, ExpressionResult};

    /// Helper to create a basic locals context with scope_stack
    fn create_locals() -> serde_json::Value {
        json!({
            "scope_stack": [
                {
                    "depth": 0,
                    "scope_type": "global",
                    "variables": {}
                }
            ]
        })
    }

    /// Helper to create locals with variables at global scope
    fn create_locals_with_vars(vars: serde_json::Value) -> serde_json::Value {
        json!({
            "scope_stack": [
                {
                    "depth": 0,
                    "scope_type": "global",
                    "variables": vars
                }
            ]
        })
    }

    /// Helper to create locals with multiple scope levels
    fn create_locals_with_scopes(scopes: Vec<serde_json::Value>) -> serde_json::Value {
        let scope_stack: Vec<_> = scopes.into_iter().enumerate().map(|(i, vars)| {
            json!({
                "depth": i,
                "scope_type": if i == 0 { "global" } else { "local" },
                "variables": vars
            })
        }).collect();

        json!({
            "scope_stack": scope_stack
        })
    }

    // ========================================================================
    // Literal Value Tests
    // ========================================================================

    #[test]
    fn test_null_literal() {
        let locals = create_locals();
        let expr = json!(null);

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert!(v.is_null()),
            ExpressionResult::Suspended(_) => panic!("Null literal should not suspend"),
        }
    }

    #[test]
    fn test_boolean_literals() {
        let locals = create_locals();

        // Test true
        let expr = json!(true);
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, true),
            ExpressionResult::Suspended(_) => panic!("Boolean literal should not suspend"),
        }

        // Test false
        let expr = json!(false);
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, false),
            ExpressionResult::Suspended(_) => panic!("Boolean literal should not suspend"),
        }
    }

    #[test]
    fn test_number_literals() {
        let locals = create_locals();

        // Integer
        let expr = json!(42);
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, 42),
            ExpressionResult::Suspended(_) => panic!("Number literal should not suspend"),
        }

        // Float
        let expr = json!(3.14);
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, 3.14),
            ExpressionResult::Suspended(_) => panic!("Number literal should not suspend"),
        }

        // Negative
        let expr = json!(-100);
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, -100),
            ExpressionResult::Suspended(_) => panic!("Number literal should not suspend"),
        }

        // Zero
        let expr = json!(0);
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, 0),
            ExpressionResult::Suspended(_) => panic!("Number literal should not suspend"),
        }
    }

    #[test]
    fn test_string_literals() {
        let locals = create_locals();

        // Simple string
        let expr = json!("hello");
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, "hello"),
            ExpressionResult::Suspended(_) => panic!("String literal should not suspend"),
        }

        // Empty string
        let expr = json!("");
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, ""),
            ExpressionResult::Suspended(_) => panic!("String literal should not suspend"),
        }

        // String with special characters
        let expr = json!("hello\nworld\t!");
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, "hello\nworld\t!"),
            ExpressionResult::Suspended(_) => panic!("String literal should not suspend"),
        }

        // Unicode
        let expr = json!("你好世界");
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, "你好世界"),
            ExpressionResult::Suspended(_) => panic!("String literal should not suspend"),
        }
    }

    // ========================================================================
    // Array Literal Tests
    // ========================================================================

    #[test]
    fn test_empty_array() {
        let locals = create_locals();
        let expr = json!([]);

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert!(v.is_array());
                assert_eq!(v.as_array().unwrap().len(), 0);
            },
            ExpressionResult::Suspended(_) => panic!("Empty array should not suspend"),
        }
    }

    #[test]
    fn test_array_with_literals() {
        let locals = create_locals();
        let expr = json!([1, "two", true, null]);

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert_eq!(v[0], 1);
                assert_eq!(v[1], "two");
                assert_eq!(v[2], true);
                assert!(v[3].is_null());
            },
            ExpressionResult::Suspended(_) => panic!("Array literal should not suspend"),
        }
    }

    #[test]
    fn test_nested_arrays() {
        let locals = create_locals();
        let expr = json!([[1, 2], [3, 4], []]);

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert_eq!(v[0][0], 1);
                assert_eq!(v[0][1], 2);
                assert_eq!(v[1][0], 3);
                assert_eq!(v[1][1], 4);
                assert!(v[2].as_array().unwrap().is_empty());
            },
            ExpressionResult::Suspended(_) => panic!("Nested array should not suspend"),
        }
    }

    #[test]
    fn test_array_with_variable_references() {
        let locals = create_locals_with_vars(json!({
            "x": 10,
            "y": 20
        }));

        let expr = json!([
            {"type": "variable", "name": "x", "depth": 0},
            {"type": "variable", "name": "y", "depth": 0},
            100
        ]);

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert_eq!(v[0], 10);
                assert_eq!(v[1], 20);
                assert_eq!(v[2], 100);
            },
            ExpressionResult::Suspended(_) => panic!("Array with variables should not suspend"),
        }
    }

    // ========================================================================
    // Object Literal Tests
    // ========================================================================

    #[test]
    fn test_empty_object() {
        let locals = create_locals();
        let expr = json!({});

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert!(v.is_object());
                assert_eq!(v.as_object().unwrap().len(), 0);
            },
            ExpressionResult::Suspended(_) => panic!("Empty object should not suspend"),
        }
    }

    #[test]
    fn test_object_with_literals() {
        let locals = create_locals();
        let expr = json!({
            "name": "Alice",
            "age": 30,
            "active": true,
            "data": null
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert_eq!(v["name"], "Alice");
                assert_eq!(v["age"], 30);
                assert_eq!(v["active"], true);
                assert!(v["data"].is_null());
            },
            ExpressionResult::Suspended(_) => panic!("Object literal should not suspend"),
        }
    }

    #[test]
    fn test_nested_objects() {
        let locals = create_locals();
        let expr = json!({
            "user": {
                "name": "Bob",
                "settings": {
                    "theme": "dark"
                }
            }
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert_eq!(v["user"]["name"], "Bob");
                assert_eq!(v["user"]["settings"]["theme"], "dark");
            },
            ExpressionResult::Suspended(_) => panic!("Nested object should not suspend"),
        }
    }

    #[test]
    fn test_object_with_variable_references() {
        let locals = create_locals_with_vars(json!({
            "username": "alice",
            "score": 100
        }));

        let expr = json!({
            "user": {"type": "variable", "name": "username", "depth": 0},
            "points": {"type": "variable", "name": "score", "depth": 0},
            "level": 5
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert_eq!(v["user"], "alice");
                assert_eq!(v["points"], 100);
                assert_eq!(v["level"], 5);
            },
            ExpressionResult::Suspended(_) => panic!("Object with variables should not suspend"),
        }
    }

    #[test]
    fn test_mixed_nested_structures() {
        let locals = create_locals_with_vars(json!({
            "items": ["a", "b", "c"]
        }));

        let expr = json!({
            "data": {
                "values": [1, 2, {"nested": true}],
                "items": {"type": "variable", "name": "items", "depth": 0}
            }
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert_eq!(v["data"]["values"][0], 1);
                assert_eq!(v["data"]["values"][2]["nested"], true);
                assert_eq!(v["data"]["items"][0], "a");
                assert_eq!(v["data"]["items"][1], "b");
                assert_eq!(v["data"]["items"][2], "c");
            },
            ExpressionResult::Suspended(_) => panic!("Mixed structures should not suspend"),
        }
    }

    // ========================================================================
    // Variable Reference Tests
    // ========================================================================

    #[test]
    fn test_simple_variable_reference() {
        let locals = create_locals_with_vars(json!({
            "x": 42
        }));

        let expr = json!({
            "type": "variable",
            "name": "x",
            "depth": 0
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, 42),
            ExpressionResult::Suspended(_) => panic!("Variable reference should not suspend"),
        }
    }

    #[test]
    fn test_undefined_variable_returns_null() {
        let locals = create_locals();

        let expr = json!({
            "type": "variable",
            "name": "undefined_var",
            "depth": 0
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert!(v.is_null()),
            ExpressionResult::Suspended(_) => panic!("Undefined variable should return null, not suspend"),
        }
    }

    #[test]
    fn test_variable_with_object_value() {
        let locals = create_locals_with_vars(json!({
            "user": {"name": "Alice", "age": 30}
        }));

        let expr = json!({
            "type": "variable",
            "name": "user",
            "depth": 0
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert_eq!(v["name"], "Alice");
                assert_eq!(v["age"], 30);
            },
            ExpressionResult::Suspended(_) => panic!("Variable reference should not suspend"),
        }
    }

    #[test]
    fn test_variable_with_array_value() {
        let locals = create_locals_with_vars(json!({
            "items": [1, 2, 3, 4, 5]
        }));

        let expr = json!({
            "type": "variable",
            "name": "items",
            "depth": 0
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert_eq!(v.as_array().unwrap().len(), 5);
                assert_eq!(v[2], 3);
            },
            ExpressionResult::Suspended(_) => panic!("Variable reference should not suspend"),
        }
    }

    #[test]
    fn test_scoped_variable_reference() {
        let locals = create_locals_with_scopes(vec![
            json!({"x": 10}),  // depth 0 (global)
            json!({"x": 20, "y": 30})   // depth 1 (local)
        ]);

        // Reference variable at depth 0
        let expr = json!({
            "type": "variable",
            "name": "x",
            "depth": 0
        });
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, 10),
            ExpressionResult::Suspended(_) => panic!("Scoped variable should not suspend"),
        }

        // Reference variable at depth 1
        let expr = json!({
            "type": "variable",
            "name": "x",
            "depth": 1
        });
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, 20),
            ExpressionResult::Suspended(_) => panic!("Scoped variable should not suspend"),
        }

        // Reference variable only in depth 1
        let expr = json!({
            "type": "variable",
            "name": "y",
            "depth": 1
        });
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, 30),
            ExpressionResult::Suspended(_) => panic!("Scoped variable should not suspend"),
        }
    }

    #[test]
    fn test_deeply_nested_scopes() {
        let locals = create_locals_with_scopes(vec![
            json!({"a": 1}),      // depth 0
            json!({"b": 2}),      // depth 1
            json!({"c": 3}),      // depth 2
            json!({"d": 4}),      // depth 3
        ]);

        for (depth, (var_name, expected_value)) in [("a", 1), ("b", 2), ("c", 3), ("d", 4)].iter().enumerate() {
            let expr = json!({
                "type": "variable",
                "name": var_name,
                "depth": depth
            });
            match evaluate_expression(&expr, &locals) {
                ExpressionResult::Value(v) => assert_eq!(v, *expected_value),
                ExpressionResult::Suspended(_) => panic!("Deep scoped variable should not suspend"),
            }
        }
    }

    // ========================================================================
    // Member Access Tests (Property Access)
    // ========================================================================
    // NOTE: These tests will initially fail until we implement proper member access
    // For now, resolve_variables handles basic cases

    #[test]
    fn test_simple_member_access() {
        let locals = create_locals_with_vars(json!({
            "user": {"name": "Alice", "age": 30}
        }));

        let expr = json!({
            "type": "member_access",
            "object": {"type": "variable", "name": "user", "depth": 0},
            "property": "name"
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, "Alice"),
            ExpressionResult::Suspended(_) => panic!("Member access should not suspend"),
        }
    }

    #[test]
    fn test_nested_member_access() {
        let locals = create_locals_with_vars(json!({
            "data": {
                "user": {
                    "profile": {
                        "name": "Bob"
                    }
                }
            }
        }));

        let expr = json!({
            "type": "member_access",
            "object": {
                "type": "member_access",
                "object": {
                    "type": "member_access",
                    "object": {"type": "variable", "name": "data", "depth": 0},
                    "property": "user"
                },
                "property": "profile"
            },
            "property": "name"
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, "Bob"),
            ExpressionResult::Suspended(_) => panic!("Nested member access should not suspend"),
        }
    }

    #[test]
    fn test_member_access_on_undefined_returns_null() {
        let locals = create_locals_with_vars(json!({
            "user": {"name": "Alice"}
        }));

        let expr = json!({
            "type": "member_access",
            "object": {"type": "variable", "name": "user", "depth": 0},
            "property": "undefined_property"
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert!(v.is_null()),
            ExpressionResult::Suspended(_) => panic!("Member access should not suspend"),
        }
    }

    #[test]
    fn test_array_index_access() {
        let locals = create_locals_with_vars(json!({
            "items": ["a", "b", "c"]
        }));

        let expr = json!({
            "type": "member_access",
            "object": {"type": "variable", "name": "items", "depth": 0},
            "property": "1"
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, "b"),
            ExpressionResult::Suspended(_) => panic!("Array index access should not suspend"),
        }
    }

    // ========================================================================
    // Edge Cases and Error Conditions
    // ========================================================================

    #[test]
    fn test_deeply_nested_structures() {
        let locals = create_locals();

        // Create a deeply nested structure
        let expr = json!({
            "level1": {
                "level2": {
                    "level3": {
                        "level4": {
                            "level5": {
                                "value": "deep"
                            }
                        }
                    }
                }
            }
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert_eq!(v["level1"]["level2"]["level3"]["level4"]["level5"]["value"], "deep");
            },
            ExpressionResult::Suspended(_) => panic!("Deep nesting should not suspend"),
        }
    }

    #[test]
    fn test_large_array() {
        let locals = create_locals();
        let large_array: Vec<i32> = (0..1000).collect();
        let expr = json!(large_array);

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                let arr = v.as_array().unwrap();
                assert_eq!(arr.len(), 1000);
                assert_eq!(arr[0], 0);
                assert_eq!(arr[999], 999);
            },
            ExpressionResult::Suspended(_) => panic!("Large array should not suspend"),
        }
    }

    #[test]
    fn test_special_characters_in_keys() {
        let locals = create_locals_with_vars(json!({
            "user-name": "alice",
            "user.email": "alice@example.com",
            "user$id": 123
        }));

        let expr = json!({
            "type": "variable",
            "name": "user-name",
            "depth": 0
        });
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, "alice"),
            ExpressionResult::Suspended(_) => panic!("Variable with special chars should not suspend"),
        }
    }

    #[test]
    fn test_expression_with_null_values() {
        let locals = create_locals_with_vars(json!({
            "a": null,
            "b": {"nested": null}
        }));

        let expr = json!({
            "x": {"type": "variable", "name": "a", "depth": 0},
            "y": {"type": "variable", "name": "b", "depth": 0}
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert!(v["x"].is_null());
                assert!(v["y"]["nested"].is_null());
            },
            ExpressionResult::Suspended(_) => panic!("Expression with nulls should not suspend"),
        }
    }

    #[test]
    fn test_mixed_types_in_array() {
        let locals = create_locals_with_vars(json!({
            "var1": "string",
            "var2": 42
        }));

        let expr = json!([
            null,
            true,
            123,
            "text",
            {"type": "variable", "name": "var1", "depth": 0},
            {"type": "variable", "name": "var2", "depth": 0},
            [1, 2, 3],
            {"key": "value"}
        ]);

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert!(v[0].is_null());
                assert_eq!(v[1], true);
                assert_eq!(v[2], 123);
                assert_eq!(v[3], "text");
                assert_eq!(v[4], "string");
                assert_eq!(v[5], 42);
                assert_eq!(v[6][1], 2);
                assert_eq!(v[7]["key"], "value");
            },
            ExpressionResult::Suspended(_) => panic!("Mixed array should not suspend"),
        }
    }

    // ========================================================================
    // Future: Binary Operations Tests
    // ========================================================================
    // These will be uncommented/updated as we implement binary operations

    /*
    #[test]
    fn test_arithmetic_addition() {
        let locals = create_locals();
        let expr = json!({
            "type": "binary_op",
            "operator": "+",
            "left": 5,
            "right": 3
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, 8),
            ExpressionResult::Suspended(_) => panic!("Arithmetic should not suspend"),
        }
    }

    #[test]
    fn test_comparison_operators() {
        let locals = create_locals();

        // Greater than
        let expr = json!({"type": "binary_op", "operator": ">", "left": 5, "right": 3});
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, true),
            _ => panic!(),
        }

        // Less than
        let expr = json!({"type": "binary_op", "operator": "<", "left": 5, "right": 3});
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, false),
            _ => panic!(),
        }

        // Equality
        let expr = json!({"type": "binary_op", "operator": "==", "left": 5, "right": 5});
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, true),
            _ => panic!(),
        }
    }

    #[test]
    fn test_logical_and() {
        let locals = create_locals();
        let expr = json!({
            "type": "logical_op",
            "operator": "&&",
            "left": true,
            "right": false
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, false),
            ExpressionResult::Suspended(_) => panic!("Logical op should not suspend"),
        }
    }

    #[test]
    fn test_logical_or() {
        let locals = create_locals();
        let expr = json!({
            "type": "logical_op",
            "operator": "||",
            "left": true,
            "right": false
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, true),
            ExpressionResult::Suspended(_) => panic!("Logical op should not suspend"),
        }
    }

    #[test]
    fn test_logical_not() {
        let locals = create_locals();
        let expr = json!({
            "type": "unary_op",
            "operator": "!",
            "operand": true
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => assert_eq!(v, false),
            ExpressionResult::Suspended(_) => panic!("Logical not should not suspend"),
        }
    }
    */

    // ========================================================================
    // Future: Function Call Tests
    // ========================================================================

    /*
    #[test]
    fn test_function_call_expression() {
        // This will need async support and DB access
        // Placeholder for future implementation
    }
    */

    // ========================================================================
    // Future: Task Expression Tests (without await)
    // ========================================================================

    /*
    #[test]
    fn test_task_run_expression() {
        let locals = create_locals();
        let expr = json!({
            "type": "task_call",
            "method": "run",
            "args": ["task_name", {"input": "value"}]
        });

        // Should return a Task structure, not suspend
        // Suspension only happens with await
        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert_eq!(v["type"], "task");
                assert_eq!(v["method"], "run");
            },
            ExpressionResult::Suspended(_) => panic!("Task expression (without await) should not suspend"),
        }
    }

    #[test]
    fn test_task_all_expression() {
        let locals = create_locals();
        let expr = json!({
            "type": "task_call",
            "method": "all",
            "args": [[
                {"type": "task_call", "method": "run", "args": ["task1", {}]},
                {"type": "task_call", "method": "run", "args": ["task2", {}]}
            ]]
        });

        match evaluate_expression(&expr, &locals) {
            ExpressionResult::Value(v) => {
                assert_eq!(v["type"], "task");
                assert_eq!(v["method"], "all");
            },
            ExpressionResult::Suspended(_) => panic!("Task.all (without await) should not suspend"),
        }
    }
    */
}
