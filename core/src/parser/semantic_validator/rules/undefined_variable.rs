//! Rule: Undefined Variable
//!
//! Reports an error when a variable is used before it's declared.
//!
//! # Examples
//!
//! ```rhythm
//! // Error: 'x' is used before declaration
//! let y = x + 1;
//! let x = 5;
//! ```
//!
//! ```rhythm
//! // OK: 'x' is declared before use
//! let x = 5;
//! let y = x + 1;
//! ```

use std::collections::HashSet;

use crate::executor::types::ast::{DeclareTarget, Expr, Stmt};
use crate::parser::WorkflowDef;

use super::super::{ValidationError, ValidationRule};

/// Rule that checks for undefined variable usage.
pub struct UndefinedVariableRule;

impl ValidationRule for UndefinedVariableRule {
    fn id(&self) -> &'static str {
        "undefined-variable"
    }

    fn description(&self) -> &'static str {
        "Variables must be declared before use"
    }

    fn validate(&self, workflow: &WorkflowDef, _source: &str) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        let mut scope = Scope::new();

        // Add built-in modules and globals
        scope.add_builtins();

        // Check the workflow body
        check_stmt(&workflow.body, &mut scope, &mut errors, self.id());

        errors
    }
}

// ============================================================================
// Scope Tracking
// ============================================================================

/// Tracks variables in scope.
struct Scope {
    /// Variables currently in scope
    defined: HashSet<String>,
}

impl Scope {
    fn new() -> Self {
        Self {
            defined: HashSet::new(),
        }
    }

    /// Add a variable to the current scope
    fn define(&mut self, name: &str) {
        self.defined.insert(name.to_string());
    }

    /// Check if a variable is defined
    fn is_defined(&self, name: &str) -> bool {
        self.defined.contains(name)
    }

    /// Add built-in modules and globals that are always available
    fn add_builtins(&mut self) {
        // Built-in modules
        self.define("task");
        self.define("timer");
        self.define("promise");
        self.define("signal");
        self.define("inputs");
        self.define("workflow");
        self.define("math");

        // Built-in operators (Rhythm represents these as function calls)
        self.define("add");
        self.define("sub");
        self.define("mul");
        self.define("div");
        self.define("eq");
        self.define("ne");
        self.define("lt");
        self.define("lte");
        self.define("gt");
        self.define("gte");
        self.define("neg");
        self.define("not");

        // Common globals
        self.define("console");
        self.define("JSON");
    }

    /// Create a child scope (for blocks, loops, etc.)
    fn child(&self) -> Self {
        Self {
            defined: self.defined.clone(),
        }
    }
}

// ============================================================================
// AST Traversal
// ============================================================================

/// Check a statement for undefined variable usage
fn check_stmt(
    stmt: &Stmt,
    scope: &mut Scope,
    errors: &mut Vec<ValidationError>,
    rule_id: &'static str,
) {
    match stmt {
        Stmt::Declare { target, init, .. } => {
            // Check the initializer FIRST (before adding variable to scope)
            // This catches: let x = x + 1;
            if let Some(init_expr) = init {
                check_expr(init_expr, scope, errors, rule_id);
            }

            // Then add the declared variable(s) to scope
            match target {
                DeclareTarget::Simple { name, .. } => {
                    scope.define(name);
                }
                DeclareTarget::Destructure { names, .. } => {
                    for name in names {
                        scope.define(name);
                    }
                }
            }
        }

        Stmt::Assign { value, .. } => {
            check_expr(value, scope, errors, rule_id);
        }

        Stmt::If {
            test,
            then_s,
            else_s,
            ..
        } => {
            check_expr(test, scope, errors, rule_id);

            // Use child scope for branches
            let mut then_scope = scope.child();
            check_stmt(then_s, &mut then_scope, errors, rule_id);

            if let Some(else_stmt) = else_s {
                let mut else_scope = scope.child();
                check_stmt(else_stmt, &mut else_scope, errors, rule_id);
            }
        }

        Stmt::While { test, body, .. } => {
            check_expr(test, scope, errors, rule_id);

            let mut body_scope = scope.child();
            check_stmt(body, &mut body_scope, errors, rule_id);
        }

        Stmt::ForLoop {
            binding,
            iterable,
            body,
            ..
        } => {
            // Check iterable in current scope
            check_expr(iterable, scope, errors, rule_id);

            // Create child scope with loop variable
            let mut body_scope = scope.child();
            body_scope.define(binding);
            check_stmt(body, &mut body_scope, errors, rule_id);
        }

        Stmt::Try {
            body,
            catch_var,
            catch_body,
            ..
        } => {
            // Check try body in child scope
            let mut try_scope = scope.child();
            check_stmt(body, &mut try_scope, errors, rule_id);

            // Check catch body with error variable in scope
            let mut catch_scope = scope.child();
            catch_scope.define(catch_var);
            check_stmt(catch_body, &mut catch_scope, errors, rule_id);
        }

        Stmt::Block { body, .. } => {
            let mut block_scope = scope.child();
            for stmt in body {
                check_stmt(stmt, &mut block_scope, errors, rule_id);
            }
        }

        Stmt::Return { value, .. } => {
            if let Some(expr) = value {
                check_expr(expr, scope, errors, rule_id);
            }
        }

        Stmt::Expr { expr, .. } => {
            check_expr(expr, scope, errors, rule_id);
        }

        // These don't contain variable references
        Stmt::Break { .. } | Stmt::Continue { .. } => {}
    }
}

/// Check an expression for undefined variable usage
fn check_expr(
    expr: &Expr,
    scope: &Scope,
    errors: &mut Vec<ValidationError>,
    rule_id: &'static str,
) {
    match expr {
        Expr::Ident { name, span } => {
            if !scope.is_defined(name) {
                errors.push(ValidationError::error(
                    *span,
                    format!("Undefined variable '{}'", name),
                    rule_id,
                ));
            }
        }

        Expr::Member { object, .. } => {
            // Only check the object, not the property
            check_expr(object, scope, errors, rule_id);
        }

        Expr::Call { callee, args, .. } => {
            check_expr(callee, scope, errors, rule_id);
            for arg in args {
                check_expr(arg, scope, errors, rule_id);
            }
        }

        Expr::Await { inner, .. } => {
            check_expr(inner, scope, errors, rule_id);
        }

        Expr::BinaryOp { left, right, .. } => {
            check_expr(left, scope, errors, rule_id);
            check_expr(right, scope, errors, rule_id);
        }

        Expr::Ternary {
            condition,
            consequent,
            alternate,
            ..
        } => {
            check_expr(condition, scope, errors, rule_id);
            check_expr(consequent, scope, errors, rule_id);
            check_expr(alternate, scope, errors, rule_id);
        }

        Expr::LitList { elements, .. } => {
            for element in elements {
                check_expr(element, scope, errors, rule_id);
            }
        }

        Expr::LitObj { properties, .. } => {
            for (_, _, value) in properties {
                check_expr(value, scope, errors, rule_id);
            }
        }

        // Literals don't contain variable references
        Expr::LitBool { .. } | Expr::LitNum { .. } | Expr::LitStr { .. } | Expr::LitNull { .. } => {
        }
    }
}
