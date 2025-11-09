//! Semantic validation for Flow v2 workflows
//!
//! This module validates WorkflowDef structures after parsing to ensure they meet
//! semantic requirements that can't be enforced by the grammar alone.

use super::WorkflowDef;

/* ===================== Error Types ===================== */

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// Workflow must be wrapped in an async function
    MissingWorkflowWrapper,
    /// Too many parameters - workflows should follow (ctx, inputs) convention
    TooManyParameters { count: usize },
    /// Reserved identifier used as parameter name
    ReservedIdentifier { name: String },
    /// Other validation errors (for future expansion)
    Custom(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::MissingWorkflowWrapper => {
                write!(f, "Workflow must be wrapped in 'async function workflow(...)' definition")
            }
            ValidationError::TooManyParameters { count } => {
                write!(
                    f,
                    "Workflow has {} parameters, but should follow (ctx, inputs) convention with at most 2 parameters",
                    count
                )
            }
            ValidationError::ReservedIdentifier { name } => {
                write!(f, "Parameter name '{}' is a reserved identifier", name)
            }
            ValidationError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ValidationError {}

pub type ValidationResult<T> = Result<T, ValidationError>;

/* ===================== Public API ===================== */

/// Reserved identifiers that cannot be used as parameter names
const RESERVED_IDENTIFIERS: &[&str] = &[
    "await", "async", "let", "const", "var", "function", "return",
    "if", "else", "for", "while", "break", "continue", "throw", "try", "catch",
    "true", "false", "null", "undefined",
];

/// Validate a workflow definition
///
/// Performs semantic validation on parsed workflows. The parser already enforces syntax,
/// so this function is reserved for semantic rules that can't be enforced by grammar.
///
/// Current rules:
/// - Enforce (ctx, inputs) convention: at most 2 parameters
/// - Reject reserved identifiers as parameter names
///
/// Future rules may include:
/// - Type checking
/// - Variable shadowing detection
/// - Async/await usage validation
/// - Stdlib function call validation
pub fn validate_workflow(workflow: &WorkflowDef) -> ValidationResult<()> {
    // Validate parameter count - enforce (ctx, inputs) convention
    if workflow.params.len() > 2 {
        return Err(ValidationError::TooManyParameters {
            count: workflow.params.len(),
        });
    }

    // Validate parameter names - reject reserved identifiers
    for param in &workflow.params {
        if RESERVED_IDENTIFIERS.contains(&param.as_str()) {
            return Err(ValidationError::ReservedIdentifier {
                name: param.clone(),
            });
        }
    }

    // Future: Add more semantic validation rules here
    // - Type checking when we add type annotations
    // - Validate that identifiers don't shadow reserved names in body
    // - Validate async/await usage
    // - Validate stdlib function calls

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::parser_v2;

    #[test]
    fn test_validate_workflow_with_ctx() {
        // Valid workflow with ctx parameter
        let source = r#"
            async function workflow(ctx) {
                return 42
            }
        "#;

        let workflow = parser_v2::parse_workflow(source).expect("Should parse");
        assert!(validate_workflow(&workflow).is_ok());
    }

    #[test]
    fn test_validate_workflow_no_params() {
        // Workflow with zero params - allowed by validator (parser rejects bare statements)
        let source = r#"
            async function workflow() {
                return 42
            }
        "#;

        let workflow = parser_v2::parse_workflow(source).expect("Should parse");
        // Currently allowed by validator - parser enforces wrapper
        assert!(validate_workflow(&workflow).is_ok());
    }

    #[test]
    fn test_validate_workflow_with_ctx_and_inputs() {
        let source = r#"
            async function workflow(ctx, inputs) {
                return inputs
            }
        "#;

        let workflow = parser_v2::parse_workflow(source).expect("Should parse");
        assert!(validate_workflow(&workflow).is_ok());
    }

    #[test]
    fn test_validate_workflow_with_member_access() {
        let source = r#"
            async function workflow(ctx, inputs) {
                return inputs.userId
            }
        "#;

        let workflow = parser_v2::parse_workflow(source).expect("Should parse");
        assert!(validate_workflow(&workflow).is_ok());
    }

    /* ===================== Validation Failure Tests ===================== */

    #[test]
    fn test_validate_rejects_too_many_parameters() {
        let source = r#"
            async function workflow(ctx, inputs, extra) {
                return 42
            }
        "#;

        let workflow = parser_v2::parse_workflow(source).expect("Should parse");
        let result = validate_workflow(&workflow);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err,
            ValidationError::TooManyParameters { count: 3 }
        );
    }

    #[test]
    fn test_validate_rejects_many_parameters() {
        let source = r#"
            async function workflow(a, b, c, d, e) {
                return 42
            }
        "#;

        let workflow = parser_v2::parse_workflow(source).expect("Should parse");
        let result = validate_workflow(&workflow);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err,
            ValidationError::TooManyParameters { count: 5 }
        );
    }

    #[test]
    fn test_validate_rejects_reserved_identifier_async() {
        let source = r#"
            async function workflow(async) {
                return 42
            }
        "#;

        let workflow = parser_v2::parse_workflow(source).expect("Should parse");
        let result = validate_workflow(&workflow);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err,
            ValidationError::ReservedIdentifier {
                name: "async".to_string()
            }
        );
    }

    #[test]
    fn test_validate_rejects_reserved_identifier_await() {
        let source = r#"
            async function workflow(ctx, await) {
                return 42
            }
        "#;

        let workflow = parser_v2::parse_workflow(source).expect("Should parse");
        let result = validate_workflow(&workflow);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err,
            ValidationError::ReservedIdentifier {
                name: "await".to_string()
            }
        );
    }

    #[test]
    fn test_validate_rejects_reserved_identifier_return() {
        let source = r#"
            async function workflow(return) {
                return 42
            }
        "#;

        let workflow = parser_v2::parse_workflow(source).expect("Should parse");
        let result = validate_workflow(&workflow);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err,
            ValidationError::ReservedIdentifier {
                name: "return".to_string()
            }
        );
    }

    #[test]
    fn test_validate_rejects_reserved_identifier_true() {
        let source = r#"
            async function workflow(true, false) {
                return 42
            }
        "#;

        let workflow = parser_v2::parse_workflow(source).expect("Should parse");
        let result = validate_workflow(&workflow);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err,
            ValidationError::ReservedIdentifier {
                name: "true".to_string()
            }
        );
    }

    #[test]
    fn test_validate_error_message_too_many_params() {
        let err = ValidationError::TooManyParameters { count: 5 };
        let msg = err.to_string();
        assert!(msg.contains("5 parameters"));
        assert!(msg.contains("(ctx, inputs)"));
    }

    #[test]
    fn test_validate_error_message_reserved() {
        let err = ValidationError::ReservedIdentifier {
            name: "async".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("async"));
        assert!(msg.contains("reserved"));
    }
}
