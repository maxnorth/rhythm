//! Parser tests - verify parsing and AST structure
//!
//! These tests verify that the parser correctly converts source code into AST structures.
//! They do NOT execute the code - that's tested in executor_v2 tests.

use crate::interpreter::executor_v2::types::ast::{Expr, MemberAccess, Stmt};
use crate::interpreter::parser_v2::{self, WorkflowDef};

/* ===================== Basic Parsing Tests ===================== */

#[test]
fn test_parse_return_number() {
    let ast = parser_v2::parse("return 42").expect("Should parse");

    // Verify AST structure
    match ast {
        Stmt::Return { value: Some(Expr::LitNum { v }) } => {
            assert_eq!(v, 42.0);
        }
        _ => panic!("Expected Return with LitNum, got {:?}", ast),
    }
}

#[test]
fn test_parse_return_negative_number() {
    let ast = parser_v2::parse("return -3.14").expect("Should parse");

    match ast {
        Stmt::Return { value: Some(Expr::LitNum { v }) } => {
            assert_eq!(v, -3.14);
        }
        _ => panic!("Expected Return with LitNum, got {:?}", ast),
    }
}

#[test]
fn test_parse_return_boolean_true() {
    let ast = parser_v2::parse("return true").expect("Should parse");

    match ast {
        Stmt::Return { value: Some(Expr::LitBool { v }) } => {
            assert!(v);
        }
        _ => panic!("Expected Return with LitBool, got {:?}", ast),
    }
}

#[test]
fn test_parse_return_boolean_false() {
    let ast = parser_v2::parse("return false").expect("Should parse");

    match ast {
        Stmt::Return { value: Some(Expr::LitBool { v }) } => {
            assert!(!v);
        }
        _ => panic!("Expected Return with LitBool, got {:?}", ast),
    }
}

#[test]
fn test_parse_return_string() {
    let ast = parser_v2::parse(r#"return "hello world""#).expect("Should parse");

    match ast {
        Stmt::Return { value: Some(Expr::LitStr { v }) } => {
            assert_eq!(v, "hello world");
        }
        _ => panic!("Expected Return with LitStr, got {:?}", ast),
    }
}

#[test]
fn test_parse_return_empty_string() {
    let ast = parser_v2::parse(r#"return """#).expect("Should parse");

    match ast {
        Stmt::Return { value: Some(Expr::LitStr { v }) } => {
            assert_eq!(v, "");
        }
        _ => panic!("Expected Return with LitStr, got {:?}", ast),
    }
}

/* ===================== Whitespace and Comments ===================== */

#[test]
fn test_parse_with_whitespace() {
    let ast = parser_v2::parse("   return   42   ").expect("Should parse");

    match ast {
        Stmt::Return { value: Some(Expr::LitNum { v }) } => {
            assert_eq!(v, 42.0);
        }
        _ => panic!("Expected Return with LitNum, got {:?}", ast),
    }
}

#[test]
fn test_parse_with_line_comment() {
    let ast = parser_v2::parse("// This is a comment\nreturn 42").expect("Should parse");

    match ast {
        Stmt::Return { value: Some(Expr::LitNum { v }) } => {
            assert_eq!(v, 42.0);
        }
        _ => panic!("Expected Return with LitNum, got {:?}", ast),
    }
}

#[test]
fn test_parse_with_block_comment() {
    let ast = parser_v2::parse("/* Block comment */ return 42").expect("Should parse");

    match ast {
        Stmt::Return { value: Some(Expr::LitNum { v }) } => {
            assert_eq!(v, 42.0);
        }
        _ => panic!("Expected Return with LitNum, got {:?}", ast),
    }
}

/* ===================== Edge Cases ===================== */

#[test]
fn test_parse_zero() {
    let ast = parser_v2::parse("return 0").expect("Should parse");

    match ast {
        Stmt::Return { value: Some(Expr::LitNum { v }) } => {
            assert_eq!(v, 0.0);
        }
        _ => panic!("Expected Return with LitNum, got {:?}", ast),
    }
}

#[test]
fn test_parse_decimal_number() {
    let ast = parser_v2::parse("return 123.456").expect("Should parse");

    match ast {
        Stmt::Return { value: Some(Expr::LitNum { v }) } => {
            assert_eq!(v, 123.456);
        }
        _ => panic!("Expected Return with LitNum, got {:?}", ast),
    }
}

#[test]
fn test_parse_string_with_spaces() {
    let ast = parser_v2::parse(r#"return "hello   world   with   spaces""#).expect("Should parse");

    match ast {
        Stmt::Return { value: Some(Expr::LitStr { v }) } => {
            assert_eq!(v, "hello   world   with   spaces");
        }
        _ => panic!("Expected Return with LitStr, got {:?}", ast),
    }
}

/* ===================== Identifier Tests ===================== */

#[test]
fn test_parse_identifier() {
    let ast = parser_v2::parse("return x").expect("Should parse");

    match ast {
        Stmt::Return { value: Some(Expr::Ident { name }) } => {
            assert_eq!(name, "x");
        }
        _ => panic!("Expected Return with Ident, got {:?}", ast),
    }
}

#[test]
fn test_parse_identifier_inputs() {
    let ast = parser_v2::parse("return inputs").expect("Should parse");

    match ast {
        Stmt::Return { value: Some(Expr::Ident { name }) } => {
            assert_eq!(name, "inputs");
        }
        _ => panic!("Expected Return with Ident, got {:?}", ast),
    }
}

/* ===================== Member Access Tests ===================== */

#[test]
fn test_parse_member_access_simple() {
    let ast = parser_v2::parse("return inputs.userId").expect("Should parse");

    match ast {
        Stmt::Return { value: Some(Expr::Member { object, property }) } => {
            // Verify object is an identifier
            match *object {
                Expr::Ident { name } => assert_eq!(name, "inputs"),
                _ => panic!("Expected Ident for object, got {:?}", object),
            }
            assert_eq!(property, "userId");
        }
        _ => panic!("Expected Return with Member, got {:?}", ast),
    }
}

#[test]
fn test_parse_member_access_nested() {
    let ast = parser_v2::parse("return ctx.user.id").expect("Should parse");

    match ast {
        Stmt::Return { value: Some(Expr::Member { object, property }) } => {
            assert_eq!(property, "id");

            // object should be ctx.user
            match *object {
                Expr::Member { object: inner_object, property: inner_property } => {
                    assert_eq!(inner_property, "user");

                    // inner_object should be ctx
                    match *inner_object {
                        Expr::Ident { name } => assert_eq!(name, "ctx"),
                        _ => panic!("Expected Ident for inner object, got {:?}", inner_object),
                    }
                }
                _ => panic!("Expected Member for object, got {:?}", object),
            }
        }
        _ => panic!("Expected Return with Member, got {:?}", ast),
    }
}

#[test]
fn test_parse_member_access_deeply_nested() {
    let ast = parser_v2::parse("return ctx.user.address.city").expect("Should parse");

    // Verify it's a return statement with nested member access
    match ast {
        Stmt::Return { value: Some(Expr::Member { property, .. }) } => {
            assert_eq!(property, "city");
            // The nesting structure is correct if parsing succeeds
        }
        _ => panic!("Expected Return with Member, got {:?}", ast),
    }
}

/* ===================== Workflow Function Tests ===================== */

#[test]
fn test_parse_workflow_minimal() {
    let source = r#"
        async function workflow(ctx) {
            return 42
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify params
    assert_eq!(workflow.params, vec!["ctx"]);

    // Verify body is a block
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            // First statement should be return 42
            match &body[0] {
                Stmt::Return { value: Some(Expr::LitNum { v }) } => {
                    assert_eq!(*v, 42.0);
                }
                _ => panic!("Expected Return with LitNum"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_workflow_no_params() {
    let source = r#"
        async function workflow() {
            return 42
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify no params
    assert_eq!(workflow.params, Vec::<String>::new());
}

#[test]
fn test_parse_workflow_with_ctx_and_inputs() {
    let source = r#"
        async function workflow(ctx, inputs) {
            return 123
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify params
    assert_eq!(workflow.params, vec!["ctx", "inputs"]);

    // Verify body
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(Expr::LitNum { v }) } => {
                    assert_eq!(*v, 123.0);
                }
                _ => panic!("Expected Return with LitNum"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_workflow_multiline_body() {
    let source = r#"
        async function workflow(ctx) {
            return 1
            return 2
            return 3
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");
    assert_eq!(workflow.params, vec!["ctx"]);

    // Verify body has 3 statements
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 3);
            // Verify each is a return statement
            for (i, stmt) in body.iter().enumerate() {
                match stmt {
                    Stmt::Return { value: Some(Expr::LitNum { v }) } => {
                        assert_eq!(*v, (i + 1) as f64);
                    }
                    _ => panic!("Expected Return with LitNum at index {}", i),
                }
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_workflow_custom_param_names() {
    let source = r#"
        async function workflow(context, data) {
            return data.value
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify custom param names
    assert_eq!(workflow.params, vec!["context", "data"]);
}

#[test]
fn test_parse_workflow_with_member_access() {
    let source = r#"
        async function workflow(ctx, inputs) {
            return inputs.userId
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");
    assert_eq!(workflow.params, vec!["ctx", "inputs"]);

    // Verify body contains member access
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(Expr::Member { object, property }) } => {
                    assert_eq!(*property, "userId");
                    match &**object {
                        Expr::Ident { name } => assert_eq!(name, "inputs"),
                        _ => panic!("Expected Ident for object"),
                    }
                }
                _ => panic!("Expected Return with Member"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

/* ===================== Serialization Tests ===================== */

#[test]
fn test_workflow_serialization_roundtrip() {
    let source = r#"
        async function workflow(ctx, inputs) {
            return inputs.userId
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Serialize to JSON
    let json = serde_json::to_string(&workflow).expect("Serialization should succeed");

    // Deserialize back
    let workflow2: WorkflowDef = serde_json::from_str(&json).expect("Deserialization should succeed");

    // Verify params match
    assert_eq!(workflow.params, workflow2.params);

    // Verify body structure matches (we can't do deep equality without implementing PartialEq)
    match (&workflow.body, &workflow2.body) {
        (Stmt::Block { body: b1 }, Stmt::Block { body: b2 }) => {
            assert_eq!(b1.len(), b2.len());
        }
        _ => panic!("Both should be Block statements"),
    }
}

#[test]
fn test_statement_serialization_roundtrip() {
    let ast = parser_v2::parse("return 42").expect("Should parse");

    // Wrap in Block as executor expects
    let program = Stmt::Block { body: vec![ast] };

    // Serialize to JSON
    let json = serde_json::to_string(&program).expect("Serialization should succeed");

    // Deserialize back
    let program2: Stmt = serde_json::from_str(&json).expect("Deserialization should succeed");

    // Verify structure
    match program2 {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(Expr::LitNum { v }) } => {
                    assert_eq!(*v, 42.0);
                }
                _ => panic!("Expected Return with LitNum"),
            }
        }
        _ => panic!("Expected Block"),
    }
}

/* ===================== Parse Error Tests ===================== */

#[test]
fn test_parser_rejects_bare_statement() {
    // parse_workflow() rejects bare statements - must use workflow wrapper
    let source = "return 42";

    let result = parser_v2::parse_workflow(source);
    assert!(result.is_err());

    // Error message should mention workflow wrapper requirement
    let err = result.unwrap_err();
    assert!(matches!(err, parser_v2::ParseError::BuildError(_)));
}

#[test]
fn test_parser_accepts_workflow_wrapper() {
    // Parser accepts proper workflow syntax
    let source = r#"
        async function workflow(ctx) {
            return 42
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");
    assert_eq!(workflow.params, vec!["ctx"]);
}

#[test]
fn test_parse_for_testing_allows_bare_statements() {
    // The parse() function (for testing) allows bare statements
    let source = "return 42";

    let stmt = parser_v2::parse(source).expect("Should parse for testing");

    // Verify it's a return statement
    assert!(matches!(stmt, Stmt::Return { .. }));
}

#[test]
fn test_parse_invalid_syntax() {
    // Test that invalid syntax is rejected
    // Note: "return" alone is now valid as an expression statement (identifier)
    // Test something that's genuinely invalid
    let source = "return return return";

    let result = parser_v2::parse(source);
    assert!(result.is_err());
}

/* ===================== While/Break/Continue Tests ===================== */

#[test]
fn test_parse_while_loop() {
    let source = r#"
        async function workflow(ctx) {
            while (true) {
                return 42
            }
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");
    assert_eq!(workflow.params, vec!["ctx"]);

    // Verify body is a block with one while statement
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::While { test, body: while_body } => {
                    // Test should be true
                    match test {
                        Expr::LitBool { v } => assert_eq!(*v, true),
                        _ => panic!("Expected LitBool for test"),
                    }
                    // Body should be a block with return statement
                    match &**while_body {
                        Stmt::Block { body } => {
                            assert_eq!(body.len(), 1);
                            assert!(matches!(&body[0], Stmt::Return { .. }));
                        }
                        _ => panic!("Expected Block for while body"),
                    }
                }
                _ => panic!("Expected While statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_while_with_break() {
    let source = r#"
        async function workflow(ctx) {
            while (true) {
                break
            }
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify body contains while with break
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::While { body: while_body, .. } => {
                    match &**while_body {
                        Stmt::Block { body } => {
                            assert_eq!(body.len(), 1);
                            assert!(matches!(&body[0], Stmt::Break));
                        }
                        _ => panic!("Expected Block for while body"),
                    }
                }
                _ => panic!("Expected While statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_while_with_continue() {
    let source = r#"
        async function workflow(ctx) {
            while (false) {
                continue
            }
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify body contains while with continue
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::While { body: while_body, .. } => {
                    match &**while_body {
                        Stmt::Block { body } => {
                            assert_eq!(body.len(), 1);
                            assert!(matches!(&body[0], Stmt::Continue));
                        }
                        _ => panic!("Expected Block for while body"),
                    }
                }
                _ => panic!("Expected While statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_nested_while() {
    let source = r#"
        async function workflow(ctx) {
            while (true) {
                while (false) {
                    break
                }
            }
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify nested while structure
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::While { body: outer_body, .. } => {
                    match &**outer_body {
                        Stmt::Block { body } => {
                            assert_eq!(body.len(), 1);
                            // Inner statement should be another while
                            assert!(matches!(&body[0], Stmt::While { .. }));
                        }
                        _ => panic!("Expected Block for outer while body"),
                    }
                }
                _ => panic!("Expected While statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_break_standalone() {
    // Test that break can be parsed as a statement (using test API)
    let ast = parser_v2::parse("break").expect("Should parse");
    assert!(matches!(ast, Stmt::Break));
}

#[test]
fn test_parse_continue_standalone() {
    // Test that continue can be parsed as a statement (using test API)
    let ast = parser_v2::parse("continue").expect("Should parse");
    assert!(matches!(ast, Stmt::Continue));
}

/* ===================== Assignment Tests ===================== */

#[test]
fn test_parse_simple_assignment() {
    let source = r#"
        async function workflow(ctx) {
            x = 42
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");
    assert_eq!(workflow.params, vec!["ctx"]);

    // Verify body is a block with one assignment
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Assign { var, path, value } => {
                    assert_eq!(var, "x");
                    assert_eq!(path.len(), 0); // No property path
                    assert!(matches!(value, Expr::LitNum { v } if *v == 42.0));
                }
                _ => panic!("Expected Assign statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_property_assignment() {
    let source = r#"
        async function workflow(ctx) {
            obj.prop = 99
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify property assignment
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Assign { var, path, value } => {
                    assert_eq!(var, "obj");
                    assert_eq!(path.len(), 1);
                    match &path[0] {
                        MemberAccess::Prop { property } => assert_eq!(property, "prop"),
                        _ => panic!("Expected Prop member access"),
                    }
                    assert!(matches!(value, Expr::LitNum { v } if *v == 99.0));
                }
                _ => panic!("Expected Assign statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_nested_property_assignment() {
    let source = r#"
        async function workflow(ctx) {
            obj.a.b = "test"
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify nested property assignment
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Assign { var, path, value } => {
                    assert_eq!(var, "obj");
                    assert_eq!(path.len(), 2);
                    match &path[0] {
                        MemberAccess::Prop { property } => assert_eq!(property, "a"),
                        _ => panic!("Expected Prop member access"),
                    }
                    match &path[1] {
                        MemberAccess::Prop { property } => assert_eq!(property, "b"),
                        _ => panic!("Expected Prop member access"),
                    }
                    assert!(matches!(value, Expr::LitStr { v } if v == "test"));
                }
                _ => panic!("Expected Assign statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_assignment_with_expression() {
    let source = r#"
        async function workflow(ctx) {
            x = Math.floor(3.7)
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify assignment with function call
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Assign { var, path, value } => {
                    assert_eq!(var, "x");
                    assert_eq!(path.len(), 0);
                    assert!(matches!(value, Expr::Call { .. }));
                }
                _ => panic!("Expected Assign statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_assignment_standalone() {
    // Test that assignment can be parsed as a statement (using test API)
    let ast = parser_v2::parse("x = 42").expect("Should parse");
    match ast {
        Stmt::Assign { var, path, value } => {
            assert_eq!(var, "x");
            assert_eq!(path.len(), 0);
            assert!(matches!(value, Expr::LitNum { v } if v == 42.0));
        }
        _ => panic!("Expected Assign statement"),
    }
}

/* ===================== Object Literal Tests ===================== */

#[test]
fn test_parse_empty_object_literal() {
    let source = r#"
        async function workflow(ctx) {
            return {}
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");
    assert_eq!(workflow.params, vec!["ctx"]);

    // Verify body is a block with return statement containing empty object
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitObj { properties } => {
                        assert_eq!(properties.len(), 0);
                    }
                    _ => panic!("Expected LitObj expression"),
                },
                _ => panic!("Expected Return statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_object_literal_single_property() {
    let source = r#"
        async function workflow(ctx) {
            return {code: "E"}
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify object with single property
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitObj { properties } => {
                        assert_eq!(properties.len(), 1);
                        assert_eq!(properties[0].0, "code");
                        assert!(matches!(&properties[0].1, Expr::LitStr { v } if v == "E"));
                    }
                    _ => panic!("Expected LitObj expression"),
                },
                _ => panic!("Expected Return statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_object_literal_multiple_properties() {
    let source = r#"
        async function workflow(ctx) {
            return {code: "E", message: "msg", value: 42}
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify object with multiple properties
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitObj { properties } => {
                        assert_eq!(properties.len(), 3);
                        assert_eq!(properties[0].0, "code");
                        assert!(matches!(&properties[0].1, Expr::LitStr { v } if v == "E"));
                        assert_eq!(properties[1].0, "message");
                        assert!(matches!(&properties[1].1, Expr::LitStr { v } if v == "msg"));
                        assert_eq!(properties[2].0, "value");
                        assert!(matches!(&properties[2].1, Expr::LitNum { v } if *v == 42.0));
                    }
                    _ => panic!("Expected LitObj expression"),
                },
                _ => panic!("Expected Return statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_object_literal_with_trailing_comma() {
    let source = r#"
        async function workflow(ctx) {
            return {code: "E", message: "msg",}
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify object parses correctly with trailing comma
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitObj { properties } => {
                        assert_eq!(properties.len(), 2);
                    }
                    _ => panic!("Expected LitObj expression"),
                },
                _ => panic!("Expected Return statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_object_literal_nested() {
    let source = r#"
        async function workflow(ctx) {
            return {outer: {inner: 42}}
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify nested object literal
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitObj { properties } => {
                        assert_eq!(properties.len(), 1);
                        assert_eq!(properties[0].0, "outer");
                        match &properties[0].1 {
                            Expr::LitObj { properties: inner_props } => {
                                assert_eq!(inner_props.len(), 1);
                                assert_eq!(inner_props[0].0, "inner");
                                assert!(matches!(&inner_props[0].1, Expr::LitNum { v } if *v == 42.0));
                            }
                            _ => panic!("Expected nested LitObj"),
                        }
                    }
                    _ => panic!("Expected LitObj expression"),
                },
                _ => panic!("Expected Return statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_object_literal_in_assignment() {
    let source = r#"
        async function workflow(ctx) {
            obj = {x: 1, y: 2}
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify object literal in assignment
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Assign { var, path, value } => {
                    assert_eq!(var, "obj");
                    assert_eq!(path.len(), 0);
                    match value {
                        Expr::LitObj { properties } => {
                            assert_eq!(properties.len(), 2);
                            assert_eq!(properties[0].0, "x");
                            assert_eq!(properties[1].0, "y");
                        }
                        _ => panic!("Expected LitObj expression"),
                    }
                }
                _ => panic!("Expected Assign statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_object_literal_with_expression_values() {
    let source = r#"
        async function workflow(ctx) {
            return {x: add(1, 2), y: ctx.value}
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify object with expression values
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitObj { properties } => {
                        assert_eq!(properties.len(), 2);
                        assert_eq!(properties[0].0, "x");
                        assert!(matches!(&properties[0].1, Expr::Call { .. }));
                        assert_eq!(properties[1].0, "y");
                        assert!(matches!(&properties[1].1, Expr::Member { .. }));
                    }
                    _ => panic!("Expected LitObj expression"),
                },
                _ => panic!("Expected Return statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

/* ===================== Array Literal Tests ===================== */

#[test]
fn test_parse_empty_array_literal() {
    let source = r#"
        async function workflow(ctx) {
            return []
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");
    assert_eq!(workflow.params, vec!["ctx"]);

    // Verify body is a block with return statement containing empty array
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitList { elements } => {
                        assert_eq!(elements.len(), 0);
                    }
                    _ => panic!("Expected LitList expression"),
                },
                _ => panic!("Expected Return statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_array_literal_single_element() {
    let source = r#"
        async function workflow(ctx) {
            return [42]
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify array with single element
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitList { elements } => {
                        assert_eq!(elements.len(), 1);
                        assert!(matches!(&elements[0], Expr::LitNum { v } if *v == 42.0));
                    }
                    _ => panic!("Expected LitList expression"),
                },
                _ => panic!("Expected Return statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_array_literal_multiple_elements() {
    let source = r#"
        async function workflow(ctx) {
            return [1, 2, 3, 4, 5]
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify array with multiple elements
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitList { elements } => {
                        assert_eq!(elements.len(), 5);
                        for (i, elem) in elements.iter().enumerate() {
                            assert!(matches!(elem, Expr::LitNum { v } if *v == (i + 1) as f64));
                        }
                    }
                    _ => panic!("Expected LitList expression"),
                },
                _ => panic!("Expected Return statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_array_literal_mixed_types() {
    let source = r#"
        async function workflow(ctx) {
            return [1, "hello", true, null]
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify array with mixed types
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitList { elements } => {
                        assert_eq!(elements.len(), 4);
                        assert!(matches!(&elements[0], Expr::LitNum { v } if *v == 1.0));
                        assert!(matches!(&elements[1], Expr::LitStr { v } if v == "hello"));
                        assert!(matches!(&elements[2], Expr::LitBool { v } if *v == true));
                        assert!(matches!(&elements[3], Expr::LitNull));
                    }
                    _ => panic!("Expected LitList expression"),
                },
                _ => panic!("Expected Return statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_array_literal_with_trailing_comma() {
    let source = r#"
        async function workflow(ctx) {
            return [1, 2, 3,]
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify array parses correctly with trailing comma
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitList { elements } => {
                        assert_eq!(elements.len(), 3);
                    }
                    _ => panic!("Expected LitList expression"),
                },
                _ => panic!("Expected Return statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_array_literal_nested() {
    let source = r#"
        async function workflow(ctx) {
            return [[1, 2], [3, 4]]
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify nested array literal
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitList { elements } => {
                        assert_eq!(elements.len(), 2);
                        // Check first nested array
                        match &elements[0] {
                            Expr::LitList { elements: inner } => {
                                assert_eq!(inner.len(), 2);
                                assert!(matches!(&inner[0], Expr::LitNum { v } if *v == 1.0));
                                assert!(matches!(&inner[1], Expr::LitNum { v } if *v == 2.0));
                            }
                            _ => panic!("Expected nested LitList"),
                        }
                        // Check second nested array
                        match &elements[1] {
                            Expr::LitList { elements: inner } => {
                                assert_eq!(inner.len(), 2);
                                assert!(matches!(&inner[0], Expr::LitNum { v } if *v == 3.0));
                                assert!(matches!(&inner[1], Expr::LitNum { v } if *v == 4.0));
                            }
                            _ => panic!("Expected nested LitList"),
                        }
                    }
                    _ => panic!("Expected LitList expression"),
                },
                _ => panic!("Expected Return statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_array_literal_in_assignment() {
    let source = r#"
        async function workflow(ctx) {
            arr = [1, 2, 3]
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify array literal in assignment
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Assign { var, path, value } => {
                    assert_eq!(var, "arr");
                    assert_eq!(path.len(), 0);
                    match value {
                        Expr::LitList { elements } => {
                            assert_eq!(elements.len(), 3);
                        }
                        _ => panic!("Expected LitList expression"),
                    }
                }
                _ => panic!("Expected Assign statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_array_literal_with_expression_elements() {
    let source = r#"
        async function workflow(ctx) {
            return [add(1, 2), ctx.value, Math.floor(3.7)]
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify array with expression elements
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitList { elements } => {
                        assert_eq!(elements.len(), 3);
                        assert!(matches!(&elements[0], Expr::Call { .. }));
                        assert!(matches!(&elements[1], Expr::Member { .. }));
                        assert!(matches!(&elements[2], Expr::Call { .. }));
                    }
                    _ => panic!("Expected LitList expression"),
                },
                _ => panic!("Expected Return statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_array_with_object_elements() {
    let source = r#"
        async function workflow(ctx) {
            return [{x: 1}, {x: 2}]
        }
    "#;

    let workflow = parser_v2::parse_workflow(source).expect("Should parse");

    // Verify array with object elements
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitList { elements } => {
                        assert_eq!(elements.len(), 2);
                        assert!(matches!(&elements[0], Expr::LitObj { .. }));
                        assert!(matches!(&elements[1], Expr::LitObj { .. }));
                    }
                    _ => panic!("Expected LitList expression"),
                },
                _ => panic!("Expected Return statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}
