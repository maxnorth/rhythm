//! Rule: Unused Variable
//!
//! Reports a warning when a variable is declared but never used.
//!
//! # Examples
//!
//! ```rhythm
//! // Warning: 'x' is declared but never used
//! let x = 5;
//! let y = 10;
//! return y;
//! ```
//!
//! # Notes
//!
//! - Variables starting with `_` are exempt (convention for intentionally unused)
//! - This is a warning, not an error, since unused variables are valid code

use std::collections::{HashMap, HashSet};

use crate::executor::types::ast::{DeclareTarget, Expr, Span, Stmt};
use crate::parser::WorkflowDef;

use super::super::{ValidationError, ValidationRule};

/// Rule that checks for unused variable declarations.
pub struct UnusedVariableRule;

impl ValidationRule for UnusedVariableRule {
    fn id(&self) -> &'static str {
        "unused-variable"
    }

    fn description(&self) -> &'static str {
        "Variables should be used after declaration"
    }

    fn validate(&self, workflow: &WorkflowDef, _source: &str) -> Vec<ValidationError> {
        // Phase 1: Collect all declarations
        let mut declarations: HashMap<String, Span> = HashMap::new();
        collect_declarations(&workflow.body, &mut declarations);

        // Phase 2: Collect all usages
        let mut usages: HashSet<String> = HashSet::new();
        collect_usages(&workflow.body, &mut usages);

        // Phase 3: Report unused declarations
        let mut errors = Vec::new();
        for (name, span) in declarations {
            // Skip variables starting with underscore (intentionally unused)
            if name.starts_with('_') {
                continue;
            }

            if !usages.contains(&name) {
                errors.push(ValidationError::warning(
                    span,
                    format!("Variable '{}' is declared but never used", name),
                    self.id(),
                ));
            }
        }

        errors
    }
}

// ============================================================================
// Declaration Collection
// ============================================================================

/// Recursively collect all variable declarations
fn collect_declarations(stmt: &Stmt, declarations: &mut HashMap<String, Span>) {
    match stmt {
        Stmt::Declare { target, .. } => match target {
            DeclareTarget::Simple { name, span } => {
                declarations.insert(name.clone(), *span);
            }
            DeclareTarget::Destructure { names, spans, .. } => {
                for (i, name) in names.iter().enumerate() {
                    let span = spans.get(i).copied().unwrap_or_default();
                    declarations.insert(name.clone(), span);
                }
            }
        },

        Stmt::ForLoop {
            binding,
            binding_span,
            body,
            ..
        } => {
            declarations.insert(binding.clone(), *binding_span);
            collect_declarations(body, declarations);
        }

        Stmt::Try {
            body,
            catch_var,
            catch_var_span,
            catch_body,
            ..
        } => {
            collect_declarations(body, declarations);
            declarations.insert(catch_var.clone(), *catch_var_span);
            collect_declarations(catch_body, declarations);
        }

        // Recurse into nested statements
        Stmt::Block { body, .. } => {
            for s in body {
                collect_declarations(s, declarations);
            }
        }

        Stmt::If { then_s, else_s, .. } => {
            collect_declarations(then_s, declarations);
            if let Some(else_stmt) = else_s {
                collect_declarations(else_stmt, declarations);
            }
        }

        Stmt::While { body, .. } => {
            collect_declarations(body, declarations);
        }

        // These don't contain declarations
        Stmt::Assign { .. }
        | Stmt::Return { .. }
        | Stmt::Expr { .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. } => {}
    }
}

// ============================================================================
// Usage Collection
// ============================================================================

/// Recursively collect all variable usages
fn collect_usages(stmt: &Stmt, usages: &mut HashSet<String>) {
    match stmt {
        Stmt::Declare { init, .. } => {
            if let Some(expr) = init {
                collect_expr_usages(expr, usages);
            }
        }

        Stmt::Assign { var, value, .. } => {
            // Assignment to a variable counts as usage
            usages.insert(var.clone());
            collect_expr_usages(value, usages);
        }

        Stmt::If {
            test,
            then_s,
            else_s,
            ..
        } => {
            collect_expr_usages(test, usages);
            collect_usages(then_s, usages);
            if let Some(else_stmt) = else_s {
                collect_usages(else_stmt, usages);
            }
        }

        Stmt::While { test, body, .. } => {
            collect_expr_usages(test, usages);
            collect_usages(body, usages);
        }

        Stmt::ForLoop { iterable, body, .. } => {
            collect_expr_usages(iterable, usages);
            collect_usages(body, usages);
        }

        Stmt::Try {
            body, catch_body, ..
        } => {
            collect_usages(body, usages);
            collect_usages(catch_body, usages);
        }

        Stmt::Block { body, .. } => {
            for s in body {
                collect_usages(s, usages);
            }
        }

        Stmt::Return { value, .. } => {
            if let Some(expr) = value {
                collect_expr_usages(expr, usages);
            }
        }

        Stmt::Expr { expr, .. } => {
            collect_expr_usages(expr, usages);
        }

        Stmt::Break { .. } | Stmt::Continue { .. } => {}
    }
}

/// Collect variable usages from an expression
fn collect_expr_usages(expr: &Expr, usages: &mut HashSet<String>) {
    match expr {
        Expr::Ident { name, .. } => {
            usages.insert(name.clone());
        }

        Expr::Member { object, .. } => {
            collect_expr_usages(object, usages);
        }

        Expr::Call { callee, args, .. } => {
            collect_expr_usages(callee, usages);
            for arg in args {
                collect_expr_usages(arg, usages);
            }
        }

        Expr::Await { inner, .. } => {
            collect_expr_usages(inner, usages);
        }

        Expr::BinaryOp { left, right, .. } => {
            collect_expr_usages(left, usages);
            collect_expr_usages(right, usages);
        }

        Expr::Ternary {
            condition,
            consequent,
            alternate,
            ..
        } => {
            collect_expr_usages(condition, usages);
            collect_expr_usages(consequent, usages);
            collect_expr_usages(alternate, usages);
        }

        Expr::LitList { elements, .. } => {
            for element in elements {
                collect_expr_usages(element, usages);
            }
        }

        Expr::LitObj { properties, .. } => {
            for (_, _, value) in properties {
                collect_expr_usages(value, usages);
            }
        }

        // Literals don't contain variable references
        Expr::LitBool { .. } | Expr::LitNum { .. } | Expr::LitStr { .. } | Expr::LitNull { .. } => {
        }
    }
}
