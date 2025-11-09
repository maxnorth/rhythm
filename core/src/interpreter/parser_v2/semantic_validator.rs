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
    /// Other validation errors (for future expansion)
    Custom(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::MissingWorkflowWrapper => {
                write!(f, "Workflow must be wrapped in 'async function workflow(...)' definition")
            }
            ValidationError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ValidationError {}

pub type ValidationResult<T> = Result<T, ValidationError>;

/* ===================== Public API ===================== */

/// Validate a workflow definition
///
/// Currently performs basic validation. The parser already enforces the workflow wrapper,
/// so this function is reserved for semantic rules that can't be enforced by grammar.
///
/// Future rules may include:
/// - Parameter count validation (recommend ctx, inputs pattern)
/// - Type checking
/// - Variable shadowing detection
/// - Async/await usage validation
/// - Stdlib function call validation
pub fn validate_workflow(workflow: &WorkflowDef) -> ValidationResult<()> {
    // Currently no semantic rules beyond what the parser enforces
    // The parser already ensures:
    // - Workflow wrapper is present (via parse_workflow rejection of bare statements)
    // - Valid syntax structure

    // Future: Add semantic validation rules here
    // Example future rules:
    // - Warn if params.len() > 2 (convention is ctx, inputs)
    // - Validate that identifiers don't shadow reserved names
    // - Type checking when we add type annotations

    let _ = workflow; // Suppress unused warning until we add validation rules

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::executor_v2::types::ast::{Expr, Stmt};

    #[test]
    fn test_validate_workflow_with_ctx() {
        // Valid workflow with ctx parameter
        let workflow = WorkflowDef {
            params: vec!["ctx".to_string()],
            body: Stmt::Block {
                body: vec![Stmt::Return {
                    value: Some(Expr::LitNum { v: 42.0 }),
                }],
            },
        };

        assert!(validate_workflow(&workflow).is_ok());
    }

    #[test]
    fn test_validate_workflow_no_params() {
        // Workflow with zero params - allowed by validator (parser rejects bare statements)
        let workflow = WorkflowDef {
            params: vec![],
            body: Stmt::Block {
                body: vec![Stmt::Return {
                    value: Some(Expr::LitNum { v: 42.0 }),
                }],
            },
        };

        // Currently allowed by validator - parser enforces wrapper
        assert!(validate_workflow(&workflow).is_ok());
    }

    #[test]
    fn test_validate_workflow_with_ctx_and_inputs() {
        let workflow = WorkflowDef {
            params: vec!["ctx".to_string(), "inputs".to_string()],
            body: Stmt::Block {
                body: vec![Stmt::Return {
                    value: Some(Expr::Ident {
                        name: "inputs".to_string(),
                    }),
                }],
            },
        };

        assert!(validate_workflow(&workflow).is_ok());
    }
}
