//! LSP Validation Adapter
//!
//! This module adapts rhythm-core's semantic validation to LSP diagnostics.
//! The actual validation rules live in core so they can be shared between
//! the LSP and the runtime.
//!
//! # Architecture
//!
//! ```text
//! rhythm-core/src/parser/semantic_validator/
//! ├── mod.rs                    # ValidationRule trait, Validator, ValidationError
//! └── rules/
//!     ├── undefined_variable.rs
//!     ├── unused_variable.rs
//!     └── unreachable_code.rs
//!
//! editors/lsp/src/validation/
//! └── mod.rs                    # This file - just converts to LSP Diagnostic
//! ```
//!
//! # Adding a New Rule
//!
//! Add new rules to `rhythm-core/src/parser/semantic_validator/rules/`.
//! They will automatically be available to both the LSP and runtime.

use rhythm_core::parser::semantic_validator::{self, Severity, ValidationError};
use tower_lsp::lsp_types::{self, DiagnosticSeverity, Position, Range};

use crate::parser::{Span, WorkflowDef};

/// Convert a core ValidationError to an LSP Diagnostic
pub fn to_lsp_diagnostic(error: &ValidationError) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic {
        range: span_to_range(&error.span),
        severity: Some(match error.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
            Severity::Hint => DiagnosticSeverity::HINT,
        }),
        code: Some(lsp_types::NumberOrString::String(error.rule_id.to_string())),
        source: Some("rhythm".to_string()),
        message: error.message.clone(),
        ..Default::default()
    }
}

/// Convert a Span to an LSP Range
fn span_to_range(span: &Span) -> Range {
    Range {
        start: Position {
            line: span.start_line.saturating_sub(1) as u32,
            character: span.start_col.saturating_sub(1) as u32,
        },
        end: Position {
            line: span.end_line.saturating_sub(1) as u32,
            character: span.end_col.saturating_sub(1) as u32,
        },
    }
}

/// Validate a workflow and return LSP diagnostics
pub fn validate_workflow(workflow: &WorkflowDef, source: &str) -> Vec<lsp_types::Diagnostic> {
    semantic_validator::validate_workflow(workflow, source)
        .iter()
        .map(to_lsp_diagnostic)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_workflow;

    #[test]
    fn test_validation_produces_lsp_diagnostics() {
        let source = "let y = x";
        let workflow = parse_workflow(source).unwrap();
        let diagnostics = validate_workflow(&workflow, source);

        assert!(!diagnostics.is_empty());
        assert_eq!(diagnostics[0].source, Some("rhythm".to_string()));
        assert!(diagnostics[0].message.contains('x'));
    }

    #[test]
    fn test_validation_severity_mapping() {
        // Undefined variable is an error
        let source = "let y = x";
        let workflow = parse_workflow(source).unwrap();
        let diagnostics = validate_workflow(&workflow, source);

        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn test_validation_warning() {
        // Unused variable is a warning
        let source = "let x = 5\nreturn 10";
        let workflow = parse_workflow(source).unwrap();
        let diagnostics = validate_workflow(&workflow, source);

        let warnings: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::WARNING))
            .collect();
        assert!(!warnings.is_empty());
    }

    #[test]
    fn test_clean_code_no_diagnostics() {
        let source = "let x = 5\nlet y = x + 1\nreturn y";
        let workflow = parse_workflow(source).unwrap();
        let diagnostics = validate_workflow(&workflow, source);

        assert!(diagnostics.is_empty(), "Clean code should have no diagnostics");
    }
}
