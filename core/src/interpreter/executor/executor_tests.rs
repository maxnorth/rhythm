/// Tests for executor helper functions
///
/// This module contains tests for resolve_variables, resolve_iterable, resolve_member_access,
/// and other executor helper functions.

#[cfg(test)]
mod tests {
    use serde_json::json;
    use super::super::expressions::{resolve_variables, resolve_iterable, resolve_member_access, lookup_scoped_variable};
    use serde_json::Value as JsonValue;

    #[test]
    fn test_resolve_variables_string() {
        let locals = json!({
            "scope_stack": [
                {
                    "depth": 0,
                    "scope_type": "global",
                    "variables": {
                        "order_id": "12345",
                        "user_name": "Alice"
                    }
                }
            ]
        });

        // Simple variable reference with scope annotation
        let input = json!({"type": "variable", "name": "order_id", "depth": 0});
        let result = resolve_variables(&input, &locals);
        assert_eq!(result, json!("12345"));

        // Regular string (not a variable)
        let input = json!("hello");
        let result = resolve_variables(&input, &locals);
        assert_eq!(result, json!("hello"));

        // Variable not found - returns null
        let input = json!({"type": "variable", "name": "missing", "depth": 0});
        let result = resolve_variables(&input, &locals);
        assert_eq!(result, JsonValue::Null);
    }

    #[test]
    fn test_resolve_variables_object() {
        let locals = json!({
            "scope_stack": [
                {
                    "depth": 0,
                    "scope_type": "global",
                    "variables": {
                        "order_id": "12345",
                        "amount": 100
                    }
                }
            ]
        });

        let input = json!({
            "order": {"type": "variable", "name": "order_id", "depth": 0},
            "total": {"type": "variable", "name": "amount", "depth": 0},
            "static": "value"
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result, json!({
            "order": "12345",
            "total": 100,
            "static": "value"
        }));
    }

    #[test]
    fn test_resolve_variables_nested() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "user_id": "user123",
                    "order_id": "order456"
                }
            }]
        });

        let input = json!({
            "user": {
                "id": {"type": "variable", "name": "user_id", "depth": 0},
                "orders": [{"type": "variable", "name": "order_id", "depth": 0}, "other"]
            },
            "count": 5
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result, json!({
            "user": {
                "id": "user123",
                "orders": ["order456", "other"]
            },
            "count": 5
        }));
    }

    #[test]
    fn test_resolve_variables_array() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "id1": "first",
                    "id2": "second"
                }
            }]
        });

        let input = json!([
            {"type": "variable", "name": "id1", "depth": 0},
            {"type": "variable", "name": "id2", "depth": 0},
            "static"
        ]);
        let result = resolve_variables(&input, &locals);

        assert_eq!(result, json!(["first", "second", "static"]));
    }

    #[test]
    fn test_resolve_variables_complex_types() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "config": {
                        "timeout": 30,
                        "retries": 3
                    }
                }
            }]
        });

        // Variable that resolves to an object
        let input = json!({
            "settings": {"type": "variable", "name": "config", "depth": 0},
            "enabled": true
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result, json!({
            "settings": {
                "timeout": 30,
                "retries": 3
            },
            "enabled": true
        }));
    }

    #[test]
    fn test_resolve_variables_empty_locals() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {}
            }]
        });

        // Variable references return null if not found
        let input = json!({
            "user_id": {"type": "variable", "name": "missing_var", "depth": 0},
            "value": 123
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result, json!({
            "user_id": JsonValue::Null,
            "value": 123
        }));
    }

    #[test]
    fn test_resolve_variables_mixed_found_and_missing() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "found": "value1"
                }
            }]
        });

        let input = json!({
            "a": {"type": "variable", "name": "found", "depth": 0},
            "b": {"type": "variable", "name": "missing", "depth": 0},
            "c": "normal"
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result, json!({
            "a": "value1",
            "b": JsonValue::Null,
            "c": "normal"
        }));
    }

    #[test]
    fn test_resolve_variables_deeply_nested() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "user_id": "user123",
                    "config": {
                        "setting1": "value1"
                    }
                }
            }]
        });

        let input = json!({
            "level1": {
                "level2": {
                    "level3": {
                        "user": {"type": "variable", "name": "user_id", "depth": 0},
                        "data": {"type": "variable", "name": "config", "depth": 0}
                    }
                }
            }
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result["level1"]["level2"]["level3"]["user"], "user123");
        assert_eq!(result["level1"]["level2"]["level3"]["data"], json!({
            "setting1": "value1"
        }));
    }

    #[test]
    fn test_resolve_variables_preserve_non_variable_dollars() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "amount": 100
                }
            }]
        });

        // Literal $ strings are just strings - no conflict with our annotation format
        let input = json!({
            "price": {"type": "variable", "name": "amount", "depth": 0},
            "currency": "USD$",
            "note": "Cost is $amount dollars"
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result["price"], 100);
        assert_eq!(result["currency"], "USD$");
        // Literal strings are preserved - no more $ variable syntax!
        assert_eq!(result["note"], "Cost is $amount dollars");
    }

    #[test]
    fn test_resolve_variables_numbers_and_primitives() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "count": 42,
                    "ratio": 3.14,
                    "enabled": true,
                    "disabled": false,
                    "empty": null
                }
            }]
        });

        let input = json!({
            "n": {"type": "variable", "name": "count", "depth": 0},
            "r": {"type": "variable", "name": "ratio", "depth": 0},
            "e": {"type": "variable", "name": "enabled", "depth": 0},
            "d": {"type": "variable", "name": "disabled", "depth": 0},
            "z": {"type": "variable", "name": "empty", "depth": 0}
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result["n"], 42);
        assert_eq!(result["r"], 3.14);
        assert_eq!(result["e"], true);
        assert_eq!(result["d"], false);
        assert_eq!(result["z"], JsonValue::Null);
    }

    #[test]
    fn test_resolve_variables_in_array_of_objects() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "id1": "first",
                    "id2": "second"
                }
            }]
        });

        let input = json!([
            { "id": {"type": "variable", "name": "id1", "depth": 0}, "name": "Item 1" },
            { "id": {"type": "variable", "name": "id2", "depth": 0}, "name": "Item 2" }
        ]);

        let result = resolve_variables(&input, &locals);

        assert_eq!(result[0]["id"], "first");
        assert_eq!(result[0]["name"], "Item 1");
        assert_eq!(result[1]["id"], "second");
        assert_eq!(result[1]["name"], "Item 2");
    }

    #[test]
    fn test_resolve_iterable_inline_array() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {}
            }]
        });

        let iterable_spec = json!({
            "type": "array",
            "value": [1, 2, 3, 4, 5]
        });

        let result = resolve_iterable(&iterable_spec, &locals).unwrap();
        assert_eq!(result, json!([1, 2, 3, 4, 5]));
    }

    #[test]
    fn test_resolve_iterable_variable_reference() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {
                    "items": ["a", "b", "c"]
                }
            }]
        });

        let iterable_spec = json!({
            "type": "variable",
            "value": {"type": "variable", "name": "items", "depth": 0}
        });

        let result = resolve_iterable(&iterable_spec, &locals).unwrap();
        assert_eq!(result, json!(["a", "b", "c"]));
    }

    #[test]
    fn test_resolve_iterable_member_access() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {}
            }],
            "inputs": {
                "orders": [
                    {"id": "order1"},
                    {"id": "order2"}
                ]
            }
        });

        let iterable_spec = json!({
            "type": "member_access",
            "value": {
                "base": "inputs",
                "path": [
                    {"type": "dot", "value": "orders"}
                ]
            }
        });

        let result = resolve_iterable(&iterable_spec, &locals).unwrap();
        assert_eq!(result, json!([
            {"id": "order1"},
            {"id": "order2"}
        ]));
    }

    #[test]
    fn test_member_access_nested_objects() {
        // Test arbitrary depth object access with dot notation
        let locals = json!({
            "inputs": {
                "user": {
                    "profile": {
                        "name": "Alice",
                        "settings": {
                            "theme": "dark"
                        }
                    }
                }
            }
        });

        // inputs.user.profile.name
        let access = json!({
            "base": "inputs",
            "path": [
                {"type": "dot", "value": "user"},
                {"type": "dot", "value": "profile"},
                {"type": "dot", "value": "name"}
            ]
        });
        let result = resolve_member_access(&access, &locals);
        assert_eq!(result, "Alice");

        // inputs.user.profile.settings.theme
        let access = json!({
            "base": "inputs",
            "path": [
                {"type": "dot", "value": "user"},
                {"type": "dot", "value": "profile"},
                {"type": "dot", "value": "settings"},
                {"type": "dot", "value": "theme"}
            ]
        });
        let result = resolve_member_access(&access, &locals);
        assert_eq!(result, "dark");
    }

    #[test]
    fn test_member_access_array_index() {
        let locals = json!({
            "items": [
                {"id": 1, "name": "first"},
                {"id": 2, "name": "second"},
                {"id": 3, "name": "third"}
            ]
        });

        // items[0]
        let access = json!({
            "base": "items",
            "path": [
                {"type": "index", "value": 0}
            ]
        });
        let result = resolve_member_access(&access, &locals);
        assert_eq!(result["id"], 1);
        assert_eq!(result["name"], "first");

        // items[1].name
        let access = json!({
            "base": "items",
            "path": [
                {"type": "index", "value": 1},
                {"type": "dot", "value": "name"}
            ]
        });
        let result = resolve_member_access(&access, &locals);
        assert_eq!(result, "second");
    }

    #[test]
    fn test_member_access_bracket_string_key() {
        let locals = json!({
            "data": {
                "0": "zero",
                "key-with-dash": "value",
                "nested": {
                    "inner": "found"
                }
            }
        });

        // data["0"] - numeric string key using brackets
        let access = json!({
            "base": "data",
            "path": [
                {"type": "bracket", "value": "0"}
            ]
        });
        let result = resolve_member_access(&access, &locals);
        assert_eq!(result, "zero");

        // data["key-with-dash"]
        let access = json!({
            "base": "data",
            "path": [
                {"type": "bracket", "value": "key-with-dash"}
            ]
        });
        let result = resolve_member_access(&access, &locals);
        assert_eq!(result, "value");

        // data["nested"]["inner"]
        let access = json!({
            "base": "data",
            "path": [
                {"type": "bracket", "value": "nested"},
                {"type": "bracket", "value": "inner"}
            ]
        });
        let result = resolve_member_access(&access, &locals);
        assert_eq!(result, "found");
    }

    #[test]
    fn test_member_access_variable_index() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "variables": {
                    "i": 1,
                    "key": "name"
                }
            }],
            "items": [
                {"name": "first"},
                {"name": "second"}
            ],
            "obj": {
                "name": "value",
                "id": 42
            }
        });

        // items[i] where i = 1
        let access = json!({
            "base": "items",
            "path": [
                {"type": "bracket_var", "value": "i"}
            ]
        });
        let result = resolve_member_access(&access, &locals);
        assert_eq!(result["name"], "second");

        // obj[key] where key = "name"
        let access = json!({
            "base": "obj",
            "path": [
                {"type": "bracket_var", "value": "key"}
            ]
        });
        let result = resolve_member_access(&access, &locals);
        assert_eq!(result, "value");
    }

    #[test]
    fn test_member_access_null_safe() {
        let locals = json!({
            "data": {
                "present": null,
                "nested": {
                    "value": "here"
                }
            }
        });

        // data.missing.deeply.nested - should return null, not error
        let access = json!({
            "base": "data",
            "path": [
                {"type": "dot", "value": "missing"},
                {"type": "dot", "value": "deeply"},
                {"type": "dot", "value": "nested"}
            ]
        });
        let result = resolve_member_access(&access, &locals);
        assert_eq!(result, JsonValue::Null);

        // data.present.field - accessing property of null
        let access = json!({
            "base": "data",
            "path": [
                {"type": "dot", "value": "present"},
                {"type": "dot", "value": "field"}
            ]
        });
        let result = resolve_member_access(&access, &locals);
        assert_eq!(result, JsonValue::Null);

        // data.nested[100] - out of bounds array access
        let access = json!({
            "base": "data",
            "path": [
                {"type": "dot", "value": "nested"},
                {"type": "index", "value": 100}
            ]
        });
        let result = resolve_member_access(&access, &locals);
        assert_eq!(result, JsonValue::Null);
    }

    #[test]
    fn test_member_access_mixed_notation() {
        let locals = json!({
            "data": {
                "items": [
                    {
                        "fields": {
                            "name": "Alice",
                            "age": 30
                        }
                    }
                ]
            }
        });

        // data.items[0].fields["name"]
        let access = json!({
            "base": "data",
            "path": [
                {"type": "dot", "value": "items"},
                {"type": "index", "value": 0},
                {"type": "dot", "value": "fields"},
                {"type": "bracket", "value": "name"}
            ]
        });
        let result = resolve_member_access(&access, &locals);
        assert_eq!(result, "Alice");
    }

    #[test]
    fn test_resolve_iterable_complex_array() {
        let locals = json!({
            "scope_stack": [{
                "depth": 0,
                "scope_type": "global",
                "variables": {}
            }]
        });

        let iterable_spec = json!({
            "type": "array",
            "value": [
                {"name": "Alice", "age": 30},
                {"name": "Bob", "age": 25},
                {"name": "Charlie", "age": 35}
            ]
        });

        let result = resolve_iterable(&iterable_spec, &locals).unwrap();
        assert_eq!(result.as_array().unwrap().len(), 3);
        assert_eq!(result[0]["name"], "Alice");
        assert_eq!(result[2]["age"], 35);
    }

    #[test]
    fn test_lookup_scoped_variable_nested_scopes() {
        // Simulate nested for loops with variables at different depths
        let locals = json!({
            "scope_stack": [
                {
                    "depth": 0,
                    "scope_type": "global",
                    "variables": {
                        "global_var": "global_value"
                    }
                },
                {
                    "depth": 1,
                    "scope_type": "for_loop",
                    "variables": {
                        "outer_item": "outer_value"
                    },
                    "metadata": {
                        "loop_variable": "outer_item",
                        "collection": ["a", "b"],
                        "current_index": 0
                    }
                },
                {
                    "depth": 2,
                    "scope_type": "for_loop",
                    "variables": {
                        "inner_item": "inner_value"
                    },
                    "metadata": {
                        "loop_variable": "inner_item",
                        "collection": [1, 2, 3],
                        "current_index": 1
                    }
                }
            ]
        });

        // Should be able to access variables at any depth
        let global = lookup_scoped_variable("global_var", 0, &locals);
        assert_eq!(global, "global_value");

        let outer = lookup_scoped_variable("outer_item", 1, &locals);
        assert_eq!(outer, "outer_value");

        let inner = lookup_scoped_variable("inner_item", 2, &locals);
        assert_eq!(inner, "inner_value");
    }

    #[test]
    fn test_resolve_variables_nested_loop_scopes() {
        // Test variable resolution with nested loop scopes
        let locals = json!({
            "scope_stack": [
                {
                    "depth": 0,
                    "scope_type": "global",
                    "variables": {
                        "userId": "user123"
                    }
                },
                {
                    "depth": 1,
                    "scope_type": "for_loop",
                    "variables": {
                        "order": {"id": "order1", "total": 100}
                    }
                },
                {
                    "depth": 2,
                    "scope_type": "for_loop",
                    "variables": {
                        "item": {"name": "Widget", "price": 25}
                    }
                }
            ]
        });

        // Build an input that references variables from all three scopes
        let input = json!({
            "user": {"type": "variable", "name": "userId", "depth": 0},
            "order": {"type": "variable", "name": "order", "depth": 1},
            "item": {"type": "variable", "name": "item", "depth": 2}
        });

        let result = resolve_variables(&input, &locals);

        assert_eq!(result["user"], "user123");
        assert_eq!(result["order"]["id"], "order1");
        assert_eq!(result["order"]["total"], 100);
        assert_eq!(result["item"]["name"], "Widget");
        assert_eq!(result["item"]["price"], 25);
    }
}
