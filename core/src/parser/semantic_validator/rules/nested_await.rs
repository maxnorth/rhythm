//! Rule: Nested Await
//!
//! Reports an error when `await` appears inside an expression rather than
//! at statement level. Rhythm requires await to be the outermost expression.
//!
//! # Valid
//!
//! ```rhythm
//! await task.run("foo", {})
//! let x = await task.run("foo", {})
//! x = await task.run("foo", {})
//! return await task.run("foo", {})
//! ```
//!
//! # Invalid
//!
//! ```rhythm
//! let x = (await task.run("foo", {})) + 1   // await inside binary op
//! foo(await bar())                           // await inside call args
//! [await foo()]                              // await inside array
//! { key: await foo() }                       // await inside object
//! if (await foo()) { }                       // await in condition
//! ```

use crate::executor::types::ast::{Expr, Stmt};
use crate::parser::WorkflowDef;

use super::super::{ValidationError, ValidationRule};

/// Rule that checks for await expressions nested inside other expressions.
pub struct NestedAwaitRule;

impl ValidationRule for NestedAwaitRule {
    fn id(&self) -> &'static str {
        "nested-await"
    }

    fn description(&self) -> &'static str {
        "await must be at statement level, not nested in expressions"
    }

    fn validate(&self, workflow: &WorkflowDef, _source: &str) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        check_stmt(&workflow.body, &mut errors, self.id());
        errors
    }
}

// ============================================================================
// AST Traversal
// ============================================================================

/// Check a statement, allowing await at "top level" positions
fn check_stmt(stmt: &Stmt, errors: &mut Vec<ValidationError>, rule_id: &'static str) {
    match stmt {
        // These ALLOW await as the outermost expression
        Stmt::Expr { expr, .. } => {
            check_top_level_expr(expr, errors, rule_id);
        }

        Stmt::Declare { init, .. } => {
            if let Some(init_expr) = init {
                check_top_level_expr(init_expr, errors, rule_id);
            }
        }

        Stmt::Assign { value, .. } => {
            check_top_level_expr(value, errors, rule_id);
        }

        Stmt::Return { value, .. } => {
            if let Some(expr) = value {
                check_top_level_expr(expr, errors, rule_id);
            }
        }

        // These DON'T allow await in their expressions
        Stmt::If {
            test,
            then_s,
            else_s,
            ..
        } => {
            check_nested_expr(test, errors, rule_id);
            check_stmt(then_s, errors, rule_id);
            if let Some(else_stmt) = else_s {
                check_stmt(else_stmt, errors, rule_id);
            }
        }

        Stmt::While { test, body, .. } => {
            check_nested_expr(test, errors, rule_id);
            check_stmt(body, errors, rule_id);
        }

        Stmt::ForLoop {
            iterable, body, ..
        } => {
            check_nested_expr(iterable, errors, rule_id);
            check_stmt(body, errors, rule_id);
        }

        Stmt::Try {
            body, catch_body, ..
        } => {
            check_stmt(body, errors, rule_id);
            check_stmt(catch_body, errors, rule_id);
        }

        Stmt::Block { body, .. } => {
            for stmt in body {
                check_stmt(stmt, errors, rule_id);
            }
        }

        // These don't contain expressions
        Stmt::Break { .. } | Stmt::Continue { .. } => {}
    }
}

/// Check a "top level" expression where await IS allowed as the outermost expr.
/// If it's an await, that's valid - but we still check inside it.
/// If it's not an await, check the whole expression for nested awaits.
fn check_top_level_expr(expr: &Expr, errors: &mut Vec<ValidationError>, rule_id: &'static str) {
    match expr {
        Expr::Await { inner, .. } => {
            // The await itself is valid here, but check inside it for nested awaits
            check_nested_expr(inner, errors, rule_id);
        }
        _ => {
            // Not an await at top level, so check entire expression for nested awaits
            check_nested_expr(expr, errors, rule_id);
        }
    }
}

/// Check an expression where await is NOT allowed.
/// Any await found here is an error.
fn check_nested_expr(expr: &Expr, errors: &mut Vec<ValidationError>, rule_id: &'static str) {
    match expr {
        Expr::Await { span, inner } => {
            errors.push(ValidationError::error(
                *span,
                "await must be at statement level, not nested in expressions",
                rule_id,
            ));
            // Continue checking inside for more nested awaits (report all errors)
            check_nested_expr(inner, errors, rule_id);
        }

        Expr::BinaryOp { left, right, .. } => {
            check_nested_expr(left, errors, rule_id);
            check_nested_expr(right, errors, rule_id);
        }

        Expr::Ternary {
            condition,
            consequent,
            alternate,
            ..
        } => {
            check_nested_expr(condition, errors, rule_id);
            check_nested_expr(consequent, errors, rule_id);
            check_nested_expr(alternate, errors, rule_id);
        }

        Expr::Call { callee, args, .. } => {
            check_nested_expr(callee, errors, rule_id);
            for arg in args {
                check_nested_expr(arg, errors, rule_id);
            }
        }

        Expr::Member { object, .. } => {
            check_nested_expr(object, errors, rule_id);
        }

        Expr::LitList { elements, .. } => {
            for element in elements {
                check_nested_expr(element, errors, rule_id);
            }
        }

        Expr::LitObj { properties, .. } => {
            for (_, _, value) in properties {
                check_nested_expr(value, errors, rule_id);
            }
        }

        // These can't contain await
        Expr::Ident { .. }
        | Expr::LitBool { .. }
        | Expr::LitNum { .. }
        | Expr::LitStr { .. }
        | Expr::LitNull { .. } => {}
    }
}
