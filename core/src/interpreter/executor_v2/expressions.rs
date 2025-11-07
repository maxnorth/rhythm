//! Expression evaluation
//!
//! Evaluates expressions to values. Currently supports only literals.
//! Future milestones will add: identifiers, member access, calls, await.

use super::types::{Expr, Val};

/// Evaluate an expression to a value
///
/// Milestone 1: Only supports literals (Bool, Num, Str)
///
/// Returns:
/// - Ok(Val) on success
/// - Err(String) on evaluation error (undefined variable, etc.)
pub fn eval_expr(expr: &Expr) -> Result<Val, String> {
    match expr {
        Expr::LitBool { v } => Ok(Val::Bool(*v)),

        Expr::LitNum { v } => Ok(Val::Num(*v)),

        Expr::LitStr { v } => Ok(Val::Str(v.clone())),

        // Not yet implemented - will add in future milestones
        Expr::Ident { .. } => Err("Identifiers not yet supported".to_string()),

        Expr::Member { .. } => Err("Member expressions not yet supported".to_string()),

        Expr::Call { .. } => Err("Call expressions not yet supported".to_string()),

        Expr::Await { .. } => Err("Await expressions not yet supported".to_string()),
    }
}
