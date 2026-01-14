//! Semantic Validation for Rhythm Workflows
//!
//! This module provides an extensible rule-based validation system that runs
//! after parsing to catch semantic errors that the grammar can't enforce.
//!
//! # Usage
//!
//! ```ignore
//! use rhythm_core::parser::{parse_workflow, semantic_validator::validate_workflow};
//!
//! let workflow = parse_workflow(source)?;
//! let errors = validate_workflow(&workflow);
//! if !errors.is_empty() {
//!     // Handle validation errors
//! }
//! ```
//!
//! # Architecture
//!
//! The validation system follows a simple pattern:
//!
//! 1. **ValidationRule trait** - Each rule implements this trait
//! 2. **Validator** - Collects and runs all rules
//! 3. **ValidationError** - The output of validation (errors, warnings, hints)
//!
//! # Adding a New Rule
//!
//! 1. Create a new file in `semantic_validator/rules/`
//! 2. Implement `ValidationRule` for your struct
//! 3. Add it to the `Validator::new()` constructor
//!
//! That's it! No other changes needed.

pub mod rules;

use crate::executor::types::ast::Span;

use super::WorkflowDef;

// ============================================================================
// Validation Error Types
// ============================================================================

/// A validation error produced by semantic analysis.
///
/// This type is independent of any specific output format (LSP, CLI, etc.)
/// so it can be used by both the runtime and tooling.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// The source location of the issue
    pub span: Span,
    /// Human-readable message
    pub message: String,
    /// Severity level
    pub severity: Severity,
    /// Which rule produced this error
    pub rule_id: &'static str,
}

/// Severity levels for validation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Must be fixed - code is incorrect
    Error,
    /// Should probably be fixed - potential bug
    Warning,
    /// Suggestion for improvement
    Hint,
}

impl ValidationError {
    /// Create a new error
    pub fn error(span: Span, message: impl Into<String>, rule_id: &'static str) -> Self {
        Self {
            span,
            message: message.into(),
            severity: Severity::Error,
            rule_id,
        }
    }

    /// Create a new warning
    pub fn warning(span: Span, message: impl Into<String>, rule_id: &'static str) -> Self {
        Self {
            span,
            message: message.into(),
            severity: Severity::Warning,
            rule_id,
        }
    }

    /// Create a new hint
    #[allow(dead_code)]
    pub fn hint(span: Span, message: impl Into<String>, rule_id: &'static str) -> Self {
        Self {
            span,
            message: message.into(),
            severity: Severity::Hint,
            rule_id,
        }
    }

    /// Check if this is an error (not a warning or hint)
    pub fn is_error(&self) -> bool {
        matches!(self.severity, Severity::Error)
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let severity = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Hint => "hint",
        };
        write!(
            f,
            "{} at line {}, col {}: {} [{}]",
            severity,
            self.span.start_line + 1,
            self.span.start_col + 1,
            self.message,
            self.rule_id
        )
    }
}

impl std::error::Error for ValidationError {}

// ============================================================================
// ValidationRule Trait
// ============================================================================

/// Trait that all validation rules must implement.
///
/// Each rule is responsible for checking one specific aspect of the code.
/// Rules should be:
/// - **Independent** - Don't depend on other rules' results
/// - **Fast** - Avoid expensive operations
/// - **Clear** - Produce helpful, actionable error messages
pub trait ValidationRule: Send + Sync {
    /// Unique identifier for this rule (e.g., "undefined-variable")
    fn id(&self) -> &'static str;

    /// Human-readable description of what this rule checks
    fn description(&self) -> &'static str;

    /// Run the validation and return any errors found.
    ///
    /// # Arguments
    /// * `workflow` - The parsed AST
    /// * `source` - The original source code (useful for additional context)
    ///
    /// # Returns
    /// A vector of validation errors. Empty vector means no issues found.
    fn validate(&self, workflow: &WorkflowDef, source: &str) -> Vec<ValidationError>;
}

// ============================================================================
// Validator - Runs All Rules
// ============================================================================

/// The main validator that orchestrates all validation rules.
pub struct Validator {
    rules: Vec<Box<dyn ValidationRule>>,
}

impl Validator {
    /// Create a new validator with all built-in rules.
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

    /// Run all validation rules and collect errors.
    pub fn validate(&self, workflow: &WorkflowDef, source: &str) -> Vec<ValidationError> {
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

// ============================================================================
// Public API
// ============================================================================

/// Validate a workflow and return all errors found.
///
/// This is the main entry point for semantic validation. It runs all
/// registered validation rules against the parsed workflow.
///
/// # Example
///
/// ```ignore
/// let workflow = parse_workflow(source)?;
/// let errors = validate_workflow(&workflow, source);
/// for error in &errors {
///     eprintln!("{}", error);
/// }
/// ```
pub fn validate_workflow(workflow: &WorkflowDef, source: &str) -> Vec<ValidationError> {
    let validator = Validator::new();
    validator.validate(workflow, source)
}

/// Check if a workflow has any validation errors (not just warnings).
///
/// This is useful for deciding whether to proceed with execution.
pub fn has_errors(workflow: &WorkflowDef, source: &str) -> bool {
    validate_workflow(workflow, source)
        .iter()
        .any(|e| e.is_error())
}

#[cfg(test)]
mod tests;
