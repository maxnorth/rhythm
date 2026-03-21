//! Semantic validation for Flow v2 workflows
//!
//! This module validates WorkflowDef structures after parsing to ensure they meet
//! semantic requirements that can't be enforced by the grammar alone.
//!
//! ## Review Notes (read this first)
//!
//! ### What this module does today
//! Nothing. The sole public function `validate_workflow` is a no-op that always
//! returns Ok(()). The reserved-identifiers list is dead code. The tests all
//! assert success on a function that cannot fail. In its current state this
//! module is scaffolding — it establishes an API boundary but provides no
//! runtime protection.
//!
//! ### How it fits into the pipeline
//! ```text
//! .flow source
//!     → PEST parser (flow.pest)        -- enforces syntax
//!     → AST builder (mod.rs)           -- produces WorkflowDef / Stmt / Expr
//!     → semantic_validator (this file)  -- should enforce meaning (currently skipped)
//!     → VM executor                    -- runs the workflow
//! ```
//!
//! ### Where it is actually called
//! - `executor/tests/helpers.rs:25` — test helper only.
//! - It is NOT called in any production path:
//!   - `services/initialization_service.rs` calls `parser::parse()` but not `validate_workflow`.
//!   - `services/workflow_service.rs` same.
//!   - `worker/runner.rs` same.
//!   This means even if validation logic were added here, production workflows
//!   would bypass it entirely until the call sites are updated.
//!
//! ### What the grammar already prevents vs. what it lets through
//! The PEST grammar (flow.pest) handles syntax: balanced braces, operator
//! precedence, keyword placement. But it cannot enforce:
//!   1. `break`/`continue` appearing only inside loops
//!   2. Reserved words used as variable names (the `identifier` rule is just
//!      `(ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")*` — it accepts "return",
//!      "if", "true", etc. as identifiers)
//!   3. `const` re-assignment (`const x = 1; x = 2` parses fine)
//!   4. Undeclared variable references
//!   5. Duplicate declarations in the same scope
//!   6. Type mismatches (not relevant yet, but will be)
//!
//! These are the gaps this validator should fill.

use super::WorkflowDef;

// REVIEW: The error type below has a single variant — `Custom(String)`. This is
// fine for a prototype, but it means every consumer has to parse the string to
// understand what went wrong. Once you add real rules, consider structured
// variants like:
//
//   ReservedIdentifier { name: String, span: Span }
//   BreakOutsideLoop { span: Span }
//   ConstReassignment { name: String, decl_span: Span, assign_span: Span }
//
// This lets callers (and future LSP tooling) match on error kind without string
// parsing, and it forces each rule to carry the source location.
//
// Also note: `ValidationError` does not carry a `Span` at all right now. Every
// other error type in the parser (`ParseError`) includes `Option<Span>` so the
// error can point to a line/column. This type should do the same.

/* ===================== Error Types ===================== */

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// Validation errors (for future expansion)
    // REVIEW: "for future expansion" is doing all the work here. This single
    // variant means the error type is effectively just a String wrapper. That's
    // okay temporarily, but the moment you add a second rule you should split
    // into specific variants that each carry a Span.
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
// REVIEW: This list exists but is never referenced outside of this constant.
// The `#[allow(dead_code)]` annotation silences the compiler warning, which
// hides the fact that this code has no effect.
//
// Some observations about the list itself:
// - It mixes JS keywords ("var", "function", "async") with Flow-specific
//   keywords. Rhythm's Flow language doesn't have `var`, `function`, or `async`
//   as user-facing keywords — check flow.pest to see what the grammar actually
//   uses. The list should match what YOUR language reserves, not JavaScript.
// - "undefined" is listed but `null` is what the grammar defines (see
//   `null_lit` rule in flow.pest). Unless you plan to add `undefined` to the
//   language, it's noise here.
// - "throw" is listed but there is no `throw` statement in the grammar — only
//   `try`/`catch`. Same for "async" — the grammar uses it only in the
//   `main_function` wrapper, not as a general keyword.
// - Missing from the list: "of" and "in" (used in for-loops), "try"/"catch"
//   (partially listed — "try" and "catch" are there but worth double-checking
//   against actual grammar usage).
//
// Recommendation: Derive this list directly from flow.pest keywords so it
// stays in sync automatically, or at minimum add a test that cross-references
// the grammar.
#[allow(dead_code)]
const RESERVED_IDENTIFIERS: &[&str] = &[
    "await",
    "async",
    "let",
    "const",
    "var",       // Not in the Flow grammar — only "let" and "const" exist
    "function",  // Only used in the `main_function` syntax sugar wrapper
    "return",
    "if",
    "else",
    "for",
    "while",
    "break",
    "continue",
    "throw",     // No throw statement exists in the grammar
    "try",
    "catch",
    "true",
    "false",
    "null",
    "undefined", // Not in the grammar — Flow has null, not undefined
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
// REVIEW: The function signature is fine — taking `&WorkflowDef` is correct
// since validation is read-only. But the implementation is empty. Here is a
// concrete sketch of what the first real pass could look like:
//
// ```rust
// pub fn validate_workflow(workflow: &WorkflowDef) -> ValidationResult<()> {
//     let mut errors = Vec::new();
//     check_reserved_identifiers(&workflow.body, &mut errors);
//     check_break_continue_placement(&workflow.body, false, &mut errors);
//     // ... more checks ...
//     if errors.is_empty() {
//         Ok(())
//     } else {
//         // Return first error, or collect all — your choice
//         Err(errors.remove(0))
//     }
// }
// ```
//
// The key design decision: do you return on first error, or collect all errors?
// Collecting all is better UX (user fixes everything in one pass) but requires
// `ValidationResult<()>` to become `Result<(), Vec<ValidationError>>` or similar.
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

    // REVIEW: All three tests below share the same problem — they only test the
    // happy path of a function that always returns Ok. There are zero negative
    // tests (tests that expect validation to REJECT something). Once you add
    // real validation rules, you'll need tests like:
    //
    //   #[test]
    //   fn test_reject_reserved_identifier_as_variable() {
    //       let source = r#"let return = 42"#;
    //       let workflow = crate::parser::parse_workflow(source).expect("Should parse");
    //       let err = validate_workflow(&workflow).unwrap_err();
    //       assert!(matches!(err, ValidationError::ReservedIdentifier { .. }));
    //   }
    //
    //   #[test]
    //   fn test_reject_break_outside_loop() {
    //       let source = r#"break"#;
    //       let workflow = crate::parser::parse_workflow(source).expect("Should parse");
    //       let err = validate_workflow(&workflow).unwrap_err();
    //       // ...
    //   }
    //
    // Until then, these tests are effectively documenting that "the no-op
    // function is still a no-op", which is not useful as a regression gate.

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
