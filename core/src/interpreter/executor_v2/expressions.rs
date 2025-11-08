//! Expression evaluation
//!
//! Evaluates expressions to values. Supports literals, identifiers, and member access.
//! Future milestones will add: calls, await.

use super::types::{Expr, Val};
use std::collections::HashMap;

/// Evaluate an expression to a value
///
/// Supports:
/// - Literals (Bool, Num, Str)
/// - Identifiers (variable lookup)
/// - Member access (object.property)
///
/// Parameters:
/// - expr: The expression to evaluate
/// - env: The variable environment for identifier lookups
///
/// Returns:
/// - Ok(Val) on success
/// - Err(String) on evaluation error (undefined variable, type mismatch, etc.)
pub fn eval_expr(expr: &Expr, env: &HashMap<String, Val>) -> Result<Val, String> {
    match expr {
        Expr::LitBool { v } => Ok(Val::Bool(*v)),

        Expr::LitNum { v } => Ok(Val::Num(*v)),

        Expr::LitStr { v } => Ok(Val::Str(v.clone())),

        Expr::Ident { name } => env
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Internal error: undefined variable '{}' (should be caught by parser/validator)", name)),

        Expr::Member { object, property } => {
            // First, evaluate the object expression
            let obj_val = eval_expr(object, env)?;

            // Then, extract the property from the object
            match obj_val {
                Val::Obj(map) => map
                    .get(property)
                    .cloned()
                    .ok_or_else(|| format!("Property '{}' not found on object", property)),
                _ => Err(format!(
                    "Cannot access property '{}' on non-object value",
                    property
                )),
            }
        }

        Expr::Call { .. } => Err("Call expressions not yet supported".to_string()),

        Expr::Await { .. } => Err("Await expressions not yet supported".to_string()),
    }
}
