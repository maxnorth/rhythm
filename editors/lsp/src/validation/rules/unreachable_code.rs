//! Rule: Unreachable Code
//!
//! Reports an error when code appears after a statement that always exits
//! (return, break, continue).
//!
//! # Examples
//!
//! ```rhythm
//! // Error: code after return is unreachable
//! return 5;
//! let x = 10;  // <-- unreachable
//! ```
//!
//! ```rhythm
//! // OK: return is in a branch, so code after is reachable
//! if (condition) {
//!     return 5;
//! }
//! let x = 10;  // <-- reachable (when condition is false)
//! ```

use crate::parser::{Span, Stmt, WorkflowDef};
use crate::validation::{Diagnostic, ValidationRule};

/// Rule that checks for unreachable code.
pub struct UnreachableCodeRule;

impl ValidationRule for UnreachableCodeRule {
    fn id(&self) -> &'static str {
        "unreachable-code"
    }

    fn description(&self) -> &'static str {
        "Code after return/break/continue is unreachable"
    }

    fn validate(&self, workflow: &WorkflowDef, _source: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        // workflow.body is typically a Block statement
        check_stmt(&workflow.body, &mut diagnostics, self.id());
        diagnostics
    }
}

/// Check a single statement (delegates to check_stmts for blocks)
fn check_stmt(stmt: &Stmt, diagnostics: &mut Vec<Diagnostic>, rule_id: &'static str) {
    match stmt {
        Stmt::Block { body, .. } => {
            check_stmts(body, diagnostics, rule_id);
        }
        _ => {
            // For non-block statements, just check children
            check_stmt_children(stmt, diagnostics, rule_id);
        }
    }
}

/// Check a list of statements for unreachable code
fn check_stmts(stmts: &[Stmt], diagnostics: &mut Vec<Diagnostic>, rule_id: &'static str) {
    let mut found_terminator = false;
    let mut terminator_span: Option<Span> = None;

    for stmt in stmts {
        if found_terminator {
            // This statement is unreachable!
            let term_name = match terminator_span {
                Some(_) => "previous statement",
                None => "a terminating statement",
            };
            diagnostics.push(Diagnostic::warning(
                stmt.span(),
                format!("Unreachable code after {}", term_name),
                rule_id,
            ));
            // Don't report every subsequent statement, just the first unreachable one
            break;
        }

        // Check if this statement is a terminator
        if is_terminator(stmt) {
            found_terminator = true;
            terminator_span = Some(stmt.span());
        }

        // Recurse into nested statements
        check_stmt_children(stmt, diagnostics, rule_id);
    }
}

/// Check if a statement always terminates (doesn't fall through)
fn is_terminator(stmt: &Stmt) -> bool {
    match stmt {
        // These always terminate
        Stmt::Return { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => true,

        // A block terminates if its last statement terminates
        Stmt::Block { body, .. } => body.last().is_some_and(is_terminator),

        // If/else terminates only if BOTH branches terminate
        Stmt::If { then_s, else_s, .. } => {
            let then_terminates = is_terminator(then_s);
            let else_terminates = else_s.as_ref().is_some_and(|s| is_terminator(s));
            then_terminates && else_terminates
        }

        // These don't guarantee termination
        Stmt::Declare { .. }
        | Stmt::Assign { .. }
        | Stmt::While { .. }
        | Stmt::ForLoop { .. }
        | Stmt::Try { .. }
        | Stmt::Expr { .. } => false,
    }
}

/// Recursively check children of a statement
fn check_stmt_children(stmt: &Stmt, diagnostics: &mut Vec<Diagnostic>, rule_id: &'static str) {
    match stmt {
        Stmt::Block { body, .. } => {
            check_stmts(body, diagnostics, rule_id);
        }

        Stmt::If { then_s, else_s, .. } => {
            check_stmt_children(then_s, diagnostics, rule_id);
            if let Some(else_stmt) = else_s {
                check_stmt_children(else_stmt, diagnostics, rule_id);
            }
        }

        Stmt::While { body, .. } => {
            check_stmt_children(body, diagnostics, rule_id);
        }

        Stmt::ForLoop { body, .. } => {
            check_stmt_children(body, diagnostics, rule_id);
        }

        Stmt::Try {
            body, catch_body, ..
        } => {
            check_stmt_children(body, diagnostics, rule_id);
            check_stmt_children(catch_body, diagnostics, rule_id);
        }

        // These don't have nested statement blocks
        Stmt::Declare { .. }
        | Stmt::Assign { .. }
        | Stmt::Return { .. }
        | Stmt::Expr { .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. } => {}
    }
}
