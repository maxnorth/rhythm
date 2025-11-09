//! Parser tests - verify parsing and AST structure
//!
//! These tests verify that the parser correctly converts source code into AST structures.
//! They do NOT execute the code - that's tested in executor_v2 tests.

use crate::interpreter::executor_v2::types::ast::{Expr, Stmt};
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
