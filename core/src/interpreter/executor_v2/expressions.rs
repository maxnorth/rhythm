//! Expression evaluation
//!
//! Evaluates expressions to values. Supports literals and variable identifiers.
//! Future milestones will add: member access, calls, await.

use super::types::{Expr, Val};
use std::collections::HashMap;

/// Evaluate an expression to a value
///
/// Supports:
/// - Literals (Bool, Num, Str)
/// - Identifiers (variable lookup)
///
/// Parameters:
/// - expr: The expression to evaluate
/// - env: The variable environment for identifier lookups
///
/// Returns:
/// - Ok(Val) on success
/// - Err(String) on evaluation error (undefined variable, etc.)
pub fn eval_expr(expr: &Expr, env: &HashMap<String, Val>) -> Result<Val, String> {
    match expr {
        Expr::LitBool { v } => Ok(Val::Bool(*v)),

        Expr::LitNum { v } => Ok(Val::Num(*v)),

        Expr::LitStr { v } => Ok(Val::Str(v.clone())),

        Expr::Ident { name } => env
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Internal error: undefined variable '{}' (should be caught by parser/validator)", name)),

        Expr::Member { .. } => Err("Member expressions not yet supported".to_string()),

        Expr::Call { .. } => Err("Call expressions not yet supported".to_string()),

        Expr::Await { .. } => Err("Await expressions not yet supported".to_string()),
    }
}
