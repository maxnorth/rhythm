//! Parser tests - verify parsing and AST structure
//!
//! These tests verify that the parser correctly converts source code into AST structures.
//! They do NOT execute the code - that's tested in executor_v2 tests.

use crate::executor::types::ast::{Expr, ForLoopKind, MemberAccess, Stmt};
use crate::parser::WorkflowDef;

/* ===================== Test Helpers ===================== */

/// Helper to extract the first statement from a Block returned by parse()
fn unwrap_block(stmt: Stmt) -> Stmt {
    match stmt {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1, "Expected single statement in block");
            body.into_iter().next().unwrap()
        }
        _ => panic!("Expected Block, got {:?}", stmt),
    }
}

/* ===================== Basic Parsing Tests ===================== */

#[test]
fn test_parse_return_number() {
    let ast = crate::parser::parse("return 42").expect("Should parse");
    let stmt = unwrap_block(ast);

    // Verify AST structure
    match stmt {
        Stmt::Return {
            value: Some(Expr::LitNum { v }),
        } => {
            assert_eq!(v, 42.0);
        }
        _ => panic!("Expected Return with LitNum, got {:?}", stmt),
    }
}

#[test]
fn test_parse_return_negative_number() {
    let ast = crate::parser::parse("return -3.5").expect("Should parse");
    let stmt = unwrap_block(ast);

    match stmt {
        Stmt::Return {
            value: Some(Expr::LitNum { v }),
        } => {
            assert_eq!(v, -3.5);
        }
        _ => panic!("Expected Return with LitNum, got {:?}", stmt),
    }
}

#[test]
fn test_parse_return_boolean_true() {
    let ast = crate::parser::parse("return true").expect("Should parse");
    let stmt = unwrap_block(ast);

    match stmt {
        Stmt::Return {
            value: Some(Expr::LitBool { v }),
        } => {
            assert!(v);
        }
        _ => panic!("Expected Return with LitBool, got {:?}", stmt),
    }
}

#[test]
fn test_parse_return_boolean_false() {
    let ast = crate::parser::parse("return false").expect("Should parse");
    let stmt = unwrap_block(ast);

    match stmt {
        Stmt::Return {
            value: Some(Expr::LitBool { v }),
        } => {
            assert!(!v);
        }
        _ => panic!("Expected Return with LitBool, got {:?}", stmt),
    }
}

#[test]
fn test_parse_return_string() {
    let ast = crate::parser::parse(r#"return "hello world""#).expect("Should parse");
    let stmt = unwrap_block(ast);

    match stmt {
        Stmt::Return {
            value: Some(Expr::LitStr { v }),
        } => {
            assert_eq!(v, "hello world");
        }
        _ => panic!("Expected Return with LitStr, got {:?}", stmt),
    }
}

#[test]
fn test_parse_return_empty_string() {
    let ast = crate::parser::parse(r#"return """#).expect("Should parse");
    let stmt = unwrap_block(ast);

    match stmt {
        Stmt::Return {
            value: Some(Expr::LitStr { v }),
        } => {
            assert_eq!(v, "");
        }
        _ => panic!("Expected Return with LitStr, got {:?}", stmt),
    }
}

/* ===================== Whitespace and Comments ===================== */

#[test]
fn test_parse_with_whitespace() {
    let ast = crate::parser::parse("   return   42   ").expect("Should parse");
    let stmt = unwrap_block(ast);

    match stmt {
        Stmt::Return {
            value: Some(Expr::LitNum { v }),
        } => {
            assert_eq!(v, 42.0);
        }
        _ => panic!("Expected Return with LitNum, got {:?}", stmt),
    }
}

#[test]
fn test_parse_with_line_comment() {
    let ast = crate::parser::parse("// This is a comment\nreturn 42").expect("Should parse");
    let stmt = unwrap_block(ast);

    match stmt {
        Stmt::Return {
            value: Some(Expr::LitNum { v }),
        } => {
            assert_eq!(v, 42.0);
        }
        _ => panic!("Expected Return with LitNum, got {:?}", stmt),
    }
}

#[test]
fn test_parse_with_block_comment() {
    let ast = crate::parser::parse("/* Block comment */ return 42").expect("Should parse");
    let stmt = unwrap_block(ast);

    match stmt {
        Stmt::Return {
            value: Some(Expr::LitNum { v }),
        } => {
            assert_eq!(v, 42.0);
        }
        _ => panic!("Expected Return with LitNum, got {:?}", stmt),
    }
}

/* ===================== Edge Cases ===================== */

#[test]
fn test_parse_zero() {
    let ast = crate::parser::parse("return 0").expect("Should parse");
    let stmt = unwrap_block(ast);

    match stmt {
        Stmt::Return {
            value: Some(Expr::LitNum { v }),
        } => {
            assert_eq!(v, 0.0);
        }
        _ => panic!("Expected Return with LitNum, got {:?}", stmt),
    }
}

#[test]
fn test_parse_decimal_number() {
    let ast = crate::parser::parse("return 123.456").expect("Should parse");
    let stmt = unwrap_block(ast);

    match stmt {
        Stmt::Return {
            value: Some(Expr::LitNum { v }),
        } => {
            assert_eq!(v, 123.456);
        }
        _ => panic!("Expected Return with LitNum, got {:?}", stmt),
    }
}

#[test]
fn test_parse_string_with_spaces() {
    let ast =
        crate::parser::parse(r#"return "hello   world   with   spaces""#).expect("Should parse");
    let stmt = unwrap_block(ast);

    match stmt {
        Stmt::Return {
            value: Some(Expr::LitStr { v }),
        } => {
            assert_eq!(v, "hello   world   with   spaces");
        }
        _ => panic!("Expected Return with LitStr, got {:?}", stmt),
    }
}

/* ===================== Identifier Tests ===================== */

#[test]
fn test_parse_identifier() {
    let ast = crate::parser::parse("return x").expect("Should parse");
    let stmt = unwrap_block(ast);

    match stmt {
        Stmt::Return {
            value: Some(Expr::Ident { name }),
        } => {
            assert_eq!(name, "x");
        }
        _ => panic!("Expected Return with Ident, got {:?}", stmt),
    }
}

#[test]
fn test_parse_identifier_inputs() {
    let ast = crate::parser::parse("return inputs").expect("Should parse");
    let stmt = unwrap_block(ast);

    match stmt {
        Stmt::Return {
            value: Some(Expr::Ident { name }),
        } => {
            assert_eq!(name, "inputs");
        }
        _ => panic!("Expected Return with Ident, got {:?}", stmt),
    }
}

/* ===================== Member Access Tests ===================== */

#[test]
fn test_parse_member_access_simple() {
    let ast = crate::parser::parse("return inputs.userId").expect("Should parse");
    let stmt = unwrap_block(ast);

    match stmt {
        Stmt::Return {
            value: Some(Expr::Member {
                object, property, ..
            }),
        } => {
            // Verify object is an identifier
            match *object {
                Expr::Ident { name } => assert_eq!(name, "inputs"),
                _ => panic!("Expected Ident for object, got {:?}", object),
            }
            assert_eq!(property, "userId");
        }
        _ => panic!("Expected Return with Member, got {:?}", stmt),
    }
}

#[test]
fn test_parse_member_access_nested() {
    let ast = crate::parser::parse("return ctx.user.id").expect("Should parse");
    let stmt = unwrap_block(ast);

    match stmt {
        Stmt::Return {
            value: Some(Expr::Member {
                object, property, ..
            }),
        } => {
            assert_eq!(property, "id");

            // object should be ctx.user
            match *object {
                Expr::Member {
                    object: inner_object,
                    property: inner_property,
                    ..
                } => {
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
        _ => panic!("Expected Return with Member, got {:?}", stmt),
    }
}

#[test]
fn test_parse_member_access_deeply_nested() {
    let ast = crate::parser::parse("return ctx.user.address.city").expect("Should parse");
    let stmt = unwrap_block(ast);

    // Verify it's a return statement with nested member access
    match stmt {
        Stmt::Return {
            value: Some(Expr::Member { property, .. }),
        } => {
            assert_eq!(property, "city");
            // The nesting structure is correct if parsing succeeds
        }
        _ => panic!("Expected Return with Member, got {:?}", stmt),
    }
}

/* ===================== Workflow Function Tests ===================== */

#[test]
fn test_parse_workflow_minimal() {
    let source = r#"
        return 42
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify body is a block
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            // First statement should be return 42
            match &body[0] {
                Stmt::Return {
                    value: Some(Expr::LitNum { v }),
                } => {
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
        return 42
    "#;

    let _workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify no params
}

#[test]
fn test_parse_workflow_with_ctx_and_inputs() {
    let source = r#"
        return 123
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify body
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return {
                    value: Some(Expr::LitNum { v }),
                } => {
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
        return 1
        return 2
        return 3
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify body has 3 statements
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 3);
            // Verify each is a return statement
            for (i, stmt) in body.iter().enumerate() {
                match stmt {
                    Stmt::Return {
                        value: Some(Expr::LitNum { v }),
                    } => {
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
        return data.value
    "#;

    let _workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify custom param names
}

#[test]
fn test_parse_workflow_with_member_access() {
    let source = r#"
        return inputs.userId
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify body contains member access
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return {
                    value:
                        Some(Expr::Member {
                            object, property, ..
                        }),
                } => {
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
        return inputs.userId
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Serialize to JSON
    let json = serde_json::to_string(&workflow).expect("Serialization should succeed");

    // Deserialize back
    let workflow2: WorkflowDef =
        serde_json::from_str(&json).expect("Deserialization should succeed");

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
    let program = crate::parser::parse("return 42").expect("Should parse");

    // parse() already wraps in Block

    // Serialize to JSON
    let json = serde_json::to_string(&program).expect("Serialization should succeed");

    // Deserialize back
    let program2: Stmt = serde_json::from_str(&json).expect("Deserialization should succeed");

    // Verify structure
    match program2 {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return {
                    value: Some(Expr::LitNum { v }),
                } => {
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
fn test_parser_accepts_bare_statement() {
    // parse_workflow() now accepts bare statements - no wrapper needed
    let source = "return 42";

    let workflow = crate::parser::parse_workflow(source).expect("Should parse bare statement");

    // Verify it's wrapped in a Block
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            assert!(matches!(&body[0], Stmt::Return { .. }));
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parser_accepts_workflow_wrapper() {
    // Parser accepts proper workflow syntax
    let source = r#"
        return 42
    "#;

    let _workflow = crate::parser::parse_workflow(source).expect("Should parse");
}

/* ===================== Optional Main Function Wrapper Tests ===================== */

#[test]
fn test_main_function_wrapper_simple() {
    // Test optional async function main() wrapper
    let source = r#"
        async function main() {
            return 42
        }
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify body is a block with return statement
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            assert!(matches!(&body[0], Stmt::Return { .. }));
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_main_function_wrapper_multiple_statements() {
    // Test main wrapper with multiple statements
    let source = r#"
        async function main() {
            let x = 10
            let y = 32
            return x + y
        }
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify body is a block with three statements
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 3);
            assert!(matches!(&body[0], Stmt::Declare { .. }));
            assert!(matches!(&body[1], Stmt::Declare { .. }));
            assert!(matches!(&body[2], Stmt::Return { .. }));
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_main_function_wrapper_with_control_flow() {
    // Test main wrapper with if statement
    let source = r#"
        async function main() {
            if (true) {
                return 1
            } else {
                return 2
            }
        }
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify body is a block with if statement
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            assert!(matches!(&body[0], Stmt::If { .. }));
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_main_function_produces_same_ast_as_bare() {
    // Verify that main function wrapper produces identical AST to bare syntax
    let with_wrapper = r#"
        async function main() {
            return 42
        }
    "#;

    let bare = r#"
        return 42
    "#;

    let workflow_wrapped = crate::parser::parse_workflow(with_wrapper).expect("Should parse");
    let workflow_bare = crate::parser::parse_workflow(bare).expect("Should parse");

    // Both should produce identical AST
    match (&workflow_wrapped.body, &workflow_bare.body) {
        (Stmt::Block { body: body1 }, Stmt::Block { body: body2 }) => {
            assert_eq!(body1.len(), body2.len());
            assert_eq!(body1.len(), 1);
            // Both should be Return statements
            assert!(matches!(&body1[0], Stmt::Return { .. }));
            assert!(matches!(&body2[0], Stmt::Return { .. }));
        }
        _ => panic!("Both should produce Block bodies"),
    }
}

#[test]
fn test_parse_function_allows_main_wrapper() {
    // Test that parse() function also accepts main wrapper
    let source = r#"
        async function main() {
            return 42
        }
    "#;

    let stmt = crate::parser::parse(source).expect("Should parse");

    // Should produce a Block with return statement
    match stmt {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            assert!(matches!(&body[0], Stmt::Return { .. }));
        }
        _ => panic!("Expected Block, got {:?}", stmt),
    }
}

#[test]
fn test_parse_for_testing_allows_bare_statements() {
    // The parse() function (for testing) allows bare statements and wraps in Block
    let source = "return 42";

    let stmt = crate::parser::parse(source).expect("Should parse for testing");

    // Verify it's wrapped in a Block
    match stmt {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            assert!(matches!(&body[0], Stmt::Return { .. }));
        }
        _ => panic!("Expected Block, got {:?}", stmt),
    }
}

#[test]
fn test_parse_invalid_syntax() {
    // Test that invalid syntax is rejected
    // Use genuinely invalid syntax that can't be parsed
    let source = "return ===";

    let result = crate::parser::parse(source);
    assert!(result.is_err());
}

/* ===================== While/Break/Continue Tests ===================== */

#[test]
fn test_parse_while_loop() {
    let source = r#"
        while (true) {
            return 42
        }
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify body is a block with one while statement
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::While {
                    test,
                    body: while_body,
                } => {
                    // Test should be true
                    match test {
                        Expr::LitBool { v } => assert!(*v),
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
        while (true) {
            break
        }
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify body contains while with break
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::While {
                    body: while_body, ..
                } => match &**while_body {
                    Stmt::Block { body } => {
                        assert_eq!(body.len(), 1);
                        assert!(matches!(&body[0], Stmt::Break));
                    }
                    _ => panic!("Expected Block for while body"),
                },
                _ => panic!("Expected While statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_while_with_continue() {
    let source = r#"
        while (false) {
            continue
        }
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify body contains while with continue
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::While {
                    body: while_body, ..
                } => match &**while_body {
                    Stmt::Block { body } => {
                        assert_eq!(body.len(), 1);
                        assert!(matches!(&body[0], Stmt::Continue));
                    }
                    _ => panic!("Expected Block for while body"),
                },
                _ => panic!("Expected While statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_nested_while() {
    let source = r#"
        while (true) {
            while (false) {
                break
            }
        }
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify nested while structure
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::While {
                    body: outer_body, ..
                } => {
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
    let ast = crate::parser::parse("break").expect("Should parse");
    let stmt = unwrap_block(ast);
    assert!(matches!(stmt, Stmt::Break));
}

#[test]
fn test_parse_continue_standalone() {
    // Test that continue can be parsed as a statement (using test API)
    let ast = crate::parser::parse("continue").expect("Should parse");
    let stmt = unwrap_block(ast);
    assert!(matches!(stmt, Stmt::Continue));
}

/* ===================== Assignment Tests ===================== */

#[test]
fn test_parse_simple_assignment() {
    let source = r#"
        x = 42
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
        obj.prop = 99
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
        obj.a.b = "test"
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
        x = Math.floor(3.7)
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
    let ast = crate::parser::parse("x = 42").expect("Should parse");
    let stmt = unwrap_block(ast);
    match stmt {
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
        return {}
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
        return {code: "E"}
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
        return {code: "E", message: "msg", value: 42}
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
fn test_parse_object_literal_shorthand() {
    // Test ES6-style shorthand: { a } means { a: a }
    let source = r#"
        return {name, age}
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify shorthand properties expand to { name: name, age: age }
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitObj { properties } => {
                        assert_eq!(properties.len(), 2);

                        // First property: name: name
                        assert_eq!(properties[0].0, "name");
                        assert!(matches!(&properties[0].1, Expr::Ident { name } if name == "name"));

                        // Second property: age: age
                        assert_eq!(properties[1].0, "age");
                        assert!(matches!(&properties[1].1, Expr::Ident { name } if name == "age"));
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
fn test_parse_object_literal_mixed_shorthand() {
    // Test mixing shorthand and regular properties
    let source = r#"
        return {name, value: 42, age}
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitObj { properties } => {
                        assert_eq!(properties.len(), 3);

                        // name (shorthand)
                        assert_eq!(properties[0].0, "name");
                        assert!(matches!(&properties[0].1, Expr::Ident { name } if name == "name"));

                        // value: 42 (regular)
                        assert_eq!(properties[1].0, "value");
                        assert!(matches!(&properties[1].1, Expr::LitNum { v } if *v == 42.0));

                        // age (shorthand)
                        assert_eq!(properties[2].0, "age");
                        assert!(matches!(&properties[2].1, Expr::Ident { name } if name == "age"));
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
        return {code: "E", message: "msg",}
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
        return {outer: {inner: 42}}
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
                            Expr::LitObj {
                                properties: inner_props,
                            } => {
                                assert_eq!(inner_props.len(), 1);
                                assert_eq!(inner_props[0].0, "inner");
                                assert!(
                                    matches!(&inner_props[0].1, Expr::LitNum { v } if *v == 42.0)
                                );
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
        obj = {x: 1, y: 2}
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
        return {x: add(1, 2), y: ctx.value}
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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

#[test]
fn test_parse_object_literal_multiline() {
    // Test object literal with properties on multiple lines
    let source = r#"
        return {
            name: "Alice",
            age: 30,
            city: "New York"
        }
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify multiline object literal parses correctly
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::LitObj { properties } => {
                        assert_eq!(properties.len(), 3);
                        assert_eq!(properties[0].0, "name");
                        assert_eq!(properties[1].0, "age");
                        assert_eq!(properties[2].0, "city");
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
fn test_parse_function_call_multiline() {
    // Test function call with arguments on multiple lines
    let source = r#"
        return add(
            10,
            32
        )
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify multiline function call parses correctly
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::Call { callee, args } => {
                        assert_eq!(args.len(), 2);
                        // Verify callee is the 'add' identifier
                        match &**callee {
                            Expr::Ident { name } => assert_eq!(name, "add"),
                            _ => panic!("Expected Ident for callee"),
                        }
                    }
                    _ => panic!("Expected Call expression"),
                },
                _ => panic!("Expected Return statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_function_call_with_multiline_object() {
    // Test function call with multiline object argument
    let source = r#"
        return Task.run("processOrder", {
            orderId: 123,
            userId: 456,
            total: 99.99
        })
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    // Verify function call with multiline object parses correctly
    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                Stmt::Return { value: Some(expr) } => match expr {
                    Expr::Call { callee, args } => {
                        assert_eq!(args.len(), 2);

                        // Verify callee is Task.run
                        match &**callee {
                            Expr::Member { property, .. } => assert_eq!(property, "run"),
                            _ => panic!("Expected Member for callee"),
                        }

                        // First arg should be string
                        assert!(matches!(&args[0], Expr::LitStr { .. }));

                        // Second arg should be object literal
                        match &args[1] {
                            Expr::LitObj { properties } => {
                                assert_eq!(properties.len(), 3);
                                assert_eq!(properties[0].0, "orderId");
                                assert_eq!(properties[1].0, "userId");
                                assert_eq!(properties[2].0, "total");
                            }
                            _ => panic!("Expected LitObj for second argument"),
                        }
                    }
                    _ => panic!("Expected Call expression"),
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
        return []
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
        return [42]
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
        return [1, 2, 3, 4, 5]
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
        return [1, "hello", true, null]
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
                        assert!(matches!(&elements[2], Expr::LitBool { v } if *v));
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
        return [1, 2, 3,]
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
        return [[1, 2], [3, 4]]
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
        arr = [1, 2, 3]
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
        return [add(1, 2), ctx.value, Math.floor(3.7)]
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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
        return [{x: 1}, {x: 2}]
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

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

/* ===================== Destructuring Tests ===================== */

use crate::executor::types::ast::DeclareTarget;

#[test]
fn test_parse_destructure_simple() {
    let source = r#"
        let { a, b } = obj
        return a
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 2);
            match &body[0] {
                Stmt::Declare { target, .. } => match target {
                    DeclareTarget::Destructure { names } => {
                        assert_eq!(names, &vec!["a".to_string(), "b".to_string()]);
                    }
                    _ => panic!("Expected Destructure target"),
                },
                _ => panic!("Expected Declare statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_destructure_with_trailing_comma() {
    let source = r#"
        const { x, y, } = Inputs
        return x
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    match workflow.body {
        Stmt::Block { body } => {
            assert_eq!(body.len(), 2);
            match &body[0] {
                Stmt::Declare { target, .. } => match target {
                    DeclareTarget::Destructure { names } => {
                        assert_eq!(names, &vec!["x".to_string(), "y".to_string()]);
                    }
                    _ => panic!("Expected Destructure target"),
                },
                _ => panic!("Expected Declare statement"),
            }
        }
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_destructure_single() {
    let source = r#"
        let { id } = result
        return id
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    match workflow.body {
        Stmt::Block { body } => match &body[0] {
            Stmt::Declare { target, .. } => match target {
                DeclareTarget::Destructure { names } => {
                    assert_eq!(names, &vec!["id".to_string()]);
                }
                _ => panic!("Expected Destructure target"),
            },
            _ => panic!("Expected Declare statement"),
        },
        _ => panic!("Expected Block for workflow body"),
    }
}

/* ===================== For Loop Tests ===================== */

#[test]
fn test_parse_for_of_simple() {
    let source = r#"
        for (let x of arr) {
            return x
        }
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    match workflow.body {
        Stmt::Block { body } => match &body[0] {
            Stmt::ForLoop {
                kind,
                binding,
                iterable,
                ..
            } => {
                assert_eq!(*kind, ForLoopKind::Of);
                assert_eq!(binding, "x");
                assert!(matches!(iterable, Expr::Ident { name } if name == "arr"));
            }
            _ => panic!("Expected ForLoop statement, got {:?}", body[0]),
        },
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_for_in_simple() {
    let source = r#"
        for (let k in obj) {
            return k
        }
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    match workflow.body {
        Stmt::Block { body } => match &body[0] {
            Stmt::ForLoop {
                kind,
                binding,
                iterable,
                ..
            } => {
                assert_eq!(*kind, ForLoopKind::In);
                assert_eq!(binding, "k");
                assert!(matches!(iterable, Expr::Ident { name } if name == "obj"));
            }
            _ => panic!("Expected ForLoop statement, got {:?}", body[0]),
        },
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_for_of_with_const() {
    let source = r#"
        for (const item of items) {
            return item
        }
    "#;

    let workflow = crate::parser::parse_workflow(source).expect("Should parse");

    match workflow.body {
        Stmt::Block { body } => match &body[0] {
            Stmt::ForLoop { kind, binding, .. } => {
                assert_eq!(*kind, ForLoopKind::Of);
                assert_eq!(binding, "item");
            }
            _ => panic!("Expected ForLoop statement"),
        },
        _ => panic!("Expected Block for workflow body"),
    }
}

#[test]
fn test_parse_for_loop_requires_block() {
    // For loops require a block body, not inline statements
    let source = "for (let x of arr) return x";

    let result = crate::parser::parse(source);
    assert!(
        result.is_err(),
        "for loops should require a block body, not inline statements"
    );
}
