//! Semantic validation for Flow v2 workflows
//!
//! This module validates WorkflowDef structures after parsing to ensure they meet
//! semantic requirements that can't be enforced by the grammar alone.

use super::WorkflowDef;

/* ===================== Error Types ===================== */

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// Validation errors (for future expansion)
    Custom(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ValidationError {}

pub type ValidationResult<T> = Result<T, ValidationError>;

/* ===================== Public API ===================== */

/// Reserved identifiers that cannot be used as parameter names
#[allow(dead_code)]
const RESERVED_IDENTIFIERS: &[&str] = &[
    "await",
    "async",
    "let",
    "const",
    "var",
    "function",
    "return",
    "if",
    "else",
    "for",
    "while",
    "break",
    "continue",
    "throw",
    "try",
    "catch",
    "true",
    "false",
    "null",
    "undefined",
];

/// Validate a workflow definition
///
/// Performs semantic validation on parsed workflows. The parser already enforces syntax,
/// so this function is reserved for semantic rules that can't be enforced by grammar.
///
/// Current rules:
/// - (Currently no validation rules - placeholder for future use)
///
/// Future rules may include:
/// - Type checking
/// - Variable shadowing detection
/// - Async/await usage validation
/// - Stdlib function call validation
pub fn validate_workflow(_workflow: &WorkflowDef) -> ValidationResult<()> {
    // Future: Add semantic validation rules here
    // - Type checking when we add type annotations
    // - Validate that identifiers don't shadow reserved names in body
    // - Validate async/await usage
    // - Validate stdlib function calls

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_workflow_simple() {
        // Valid workflow with bare syntax
        let source = r#"
            return 42
        "#;

        let workflow = crate::parser::parse_workflow(source).expect("Should parse");
        assert!(validate_workflow(&workflow).is_ok());
    }

    #[test]
    fn test_validate_workflow_with_front_matter() {
        // Valid workflow with front matter
        let source = r#"
```
name: test_workflow
```
let x = 42
return x
        "#;

        let workflow = crate::parser::parse_workflow(source).expect("Should parse");
        assert!(validate_workflow(&workflow).is_ok());
        assert!(workflow.front_matter.is_some());
    }

    #[test]
    fn test_validate_workflow_complex() {
        // Valid workflow with multiple statements
        let source = r#"
            let x = 42
            let y = x + 10
            return y
        "#;

        let workflow = crate::parser::parse_workflow(source).expect("Should parse");
        assert!(validate_workflow(&workflow).is_ok());
    }
}
