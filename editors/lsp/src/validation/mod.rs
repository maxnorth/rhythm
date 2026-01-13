//! Semantic Validation for Rhythm Language
//!
//! This module provides an extensible rule-based validation system.
//!
//! # Architecture
//!
//! The validation system follows a simple pattern:
//!
//! 1. **ValidationRule trait** - Each rule implements this trait
//! 2. **Validator** - Collects and runs all rules
//! 3. **Diagnostic** - The output of validation (errors, warnings, hints)
//!
//! # Adding a New Rule
//!
//! 1. Create a new file in `validation/rules/`
//! 2. Implement `ValidationRule` for your struct
//! 3. Add it to the `Validator::new()` constructor
//!
//! That's it! No other changes needed.
//!
//! # Example
//!
//! ```ignore
//! pub struct MyNewRule;
//!
//! impl ValidationRule for MyNewRule {
//!     fn id(&self) -> &'static str { "my-new-rule" }
//!     fn description(&self) -> &'static str { "Checks for something" }
//!
//!     fn validate(&self, workflow: &WorkflowDef, source: &str) -> Vec<Diagnostic> {
//!         // Your validation logic here
//!         vec![]
//!     }
//! }
//! ```

pub mod rules;

use crate::parser::{Span, WorkflowDef};
use tower_lsp::lsp_types::{self, DiagnosticSeverity, Position, Range};

/// A diagnostic message produced by validation.
///
/// This maps directly to LSP's Diagnostic type but is decoupled
/// so validation logic doesn't depend on LSP types.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// The source location of the issue
    pub span: Span,
    /// Human-readable message
    pub message: String,
    /// Severity level
    pub severity: Severity,
    /// Which rule produced this diagnostic
    pub rule_id: &'static str,
}

/// Severity levels for diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Must be fixed - code is incorrect
    Error,
    /// Should probably be fixed - potential bug
    Warning,
    /// Suggestion for improvement
    Hint,
}

impl Diagnostic {
    /// Create a new error diagnostic
    pub fn error(span: Span, message: impl Into<String>, rule_id: &'static str) -> Self {
        Self {
            span,
            message: message.into(),
            severity: Severity::Error,
            rule_id,
        }
    }

    /// Create a new warning diagnostic
    pub fn warning(span: Span, message: impl Into<String>, rule_id: &'static str) -> Self {
        Self {
            span,
            message: message.into(),
            severity: Severity::Warning,
            rule_id,
        }
    }

    /// Create a new hint diagnostic
    #[allow(dead_code)]
    pub fn hint(span: Span, message: impl Into<String>, rule_id: &'static str) -> Self {
        Self {
            span,
            message: message.into(),
            severity: Severity::Hint,
            rule_id,
        }
    }

    /// Convert to LSP Diagnostic type
    pub fn to_lsp_diagnostic(&self) -> lsp_types::Diagnostic {
        lsp_types::Diagnostic {
            range: span_to_range(&self.span),
            severity: Some(match self.severity {
                Severity::Error => DiagnosticSeverity::ERROR,
                Severity::Warning => DiagnosticSeverity::WARNING,
                Severity::Hint => DiagnosticSeverity::HINT,
            }),
            code: Some(lsp_types::NumberOrString::String(self.rule_id.to_string())),
            source: Some("rhythm".to_string()),
            message: self.message.clone(),
            ..Default::default()
        }
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

// ============================================================================
// ValidationRule Trait
// ============================================================================

/// Trait that all validation rules must implement.
///
/// Each rule is responsible for checking one specific aspect of the code.
/// Rules should be:
/// - **Independent** - Don't depend on other rules' results
/// - **Fast** - Avoid expensive operations; validation runs on every edit
/// - **Clear** - Produce helpful, actionable error messages
pub trait ValidationRule: Send + Sync {
    /// Unique identifier for this rule (e.g., "undefined-variable")
    ///
    /// This appears in the diagnostic's `code` field in the editor.
    fn id(&self) -> &'static str;

    /// Human-readable description of what this rule checks
    fn description(&self) -> &'static str;

    /// Run the validation and return any diagnostics found.
    ///
    /// # Arguments
    /// * `workflow` - The parsed AST
    /// * `source` - The original source code (useful for additional context)
    ///
    /// # Returns
    /// A vector of diagnostics. Empty vector means no issues found.
    fn validate(&self, workflow: &WorkflowDef, source: &str) -> Vec<Diagnostic>;
}

// ============================================================================
// Validator - Runs All Rules
// ============================================================================

/// The main validator that orchestrates all validation rules.
///
/// # Usage
///
/// ```ignore
/// let validator = Validator::new();
/// let diagnostics = validator.validate(&workflow, source);
/// ```
pub struct Validator {
    rules: Vec<Box<dyn ValidationRule>>,
}

impl Validator {
    /// Create a new validator with all built-in rules.
    ///
    /// To add a new rule, just add it to this list!
    pub fn new() -> Self {
        Self {
            rules: vec![
                // Error rules - these indicate bugs
                Box::new(rules::UndefinedVariableRule),
                Box::new(rules::UnreachableCodeRule),
                // Warning rules - these are suggestions
                Box::new(rules::UnusedVariableRule),
            ],
        }
    }

    /// Run all validation rules and collect diagnostics.
    pub fn validate(&self, workflow: &WorkflowDef, source: &str) -> Vec<Diagnostic> {
        self.rules
            .iter()
            .flat_map(|rule| rule.validate(workflow, source))
            .collect()
    }

    /// Get a list of all registered rules (useful for documentation)
    #[allow(dead_code)]
    pub fn rules(&self) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
        self.rules.iter().map(|r| (r.id(), r.description()))
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
