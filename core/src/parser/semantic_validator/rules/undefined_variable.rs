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
///
/// In Rhythm, variables can be declared in two ways:
/// - `let x = 1` - block-scoped, removed when block exits
/// - `x = 1` - function-scoped, persists outside blocks
///
/// This struct tracks both types to properly handle nested scopes.
struct Scope {
    /// Variables currently visible (includes inherited from parent)
    defined: HashSet<String>,
    /// Variables declared with `let` in the current block (to remove on exit)
    block_local: HashSet<String>,
}

impl Scope {
    fn new() -> Self {
        Self {
            defined: HashSet::new(),
            block_local: HashSet::new(),
        }
    }

    /// Add a variable declared with `let` (block-scoped)
    fn define_let(&mut self, name: &str) {
        self.defined.insert(name.to_string());
        self.block_local.insert(name.to_string());
    }

    /// Add a variable from simple assignment (function-scoped)
    fn define(&mut self, name: &str) {
        self.defined.insert(name.to_string());
        // NOT added to block_local - persists outside blocks
    }

    /// Check if a variable is defined
    fn is_defined(&self, name: &str) -> bool {
        self.defined.contains(name)
    }

    /// Add built-in modules and globals that are always available
    fn add_builtins(&mut self) {
        // Built-in modules (lowercase is canonical, uppercase for legacy tests)
        self.define("task");
        self.define("Task");
        self.define("timer");
        self.define("Timer");
        self.define("promise");
        self.define("Promise");
        self.define("signal");
        self.define("Signal");
        self.define("inputs");
        self.define("Inputs");
        self.define("workflow");
        self.define("Workflow");
        self.define("math");
        self.define("Math");
        self.define("ctx");
        self.define("Ctx");
        self.define("Context");

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

        // Control flow
        self.define("throw");

        // Common globals
        self.define("console");
        self.define("JSON");
    }

    /// Enter a child block scope
    ///
    /// Returns the set of block-local variables to restore on exit
    fn enter_block(&mut self) -> HashSet<String> {
        std::mem::take(&mut self.block_local)
    }

    /// Exit a child block scope
    ///
    /// Removes block-local variables and restores parent's block_local set
    fn exit_block(&mut self, parent_block_local: HashSet<String>) {
        // Remove variables that were declared with `let` in this block
        for var in &self.block_local {
            self.defined.remove(var);
        }
        // Restore parent's block_local tracking
        self.block_local = parent_block_local;
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

            // Then add the declared variable(s) to scope (block-scoped with `let`)
            match target {
                DeclareTarget::Simple { name, .. } => {
                    scope.define_let(name);
                }
                DeclareTarget::Destructure { names, .. } => {
                    for name in names {
                        scope.define_let(name);
                    }
                }
            }
        }

        Stmt::Assign { var, path, value, .. } => {
            // Check the value expression first
            check_expr(value, scope, errors, rule_id);

            // If path is empty (simple assignment like `x = 42`), this creates the variable
            // If path is non-empty (like `obj.prop = 42`), the base variable must exist
            if path.is_empty() {
                // Simple assignment creates/updates the variable (function-scoped)
                scope.define(var);
            } else {
                // Property/index assignment - base variable must be defined
                if !scope.is_defined(var) {
                    // Note: we don't report error here - the runtime would,
                    // but semantically this could be caught by flow analysis
                }
            }
        }

        Stmt::If {
            test,
            then_s,
            else_s,
            ..
        } => {
            check_expr(test, scope, errors, rule_id);

            // Enter block scope for then branch
            let saved = scope.enter_block();
            check_stmt(then_s, scope, errors, rule_id);
            scope.exit_block(saved);

            if let Some(else_stmt) = else_s {
                let saved = scope.enter_block();
                check_stmt(else_stmt, scope, errors, rule_id);
                scope.exit_block(saved);
            }
        }

        Stmt::While { test, body, .. } => {
            check_expr(test, scope, errors, rule_id);

            let saved = scope.enter_block();
            check_stmt(body, scope, errors, rule_id);
            scope.exit_block(saved);
        }

        Stmt::ForLoop {
            binding,
            iterable,
            body,
            ..
        } => {
            // Check iterable in current scope
            check_expr(iterable, scope, errors, rule_id);

            // Enter block scope with loop variable (loop variable is block-scoped)
            let saved = scope.enter_block();
            scope.define_let(binding);
            check_stmt(body, scope, errors, rule_id);
            scope.exit_block(saved);
        }

        Stmt::Try {
            body,
            catch_var,
            catch_body,
            ..
        } => {
            // Check try body in block scope
            let saved = scope.enter_block();
            check_stmt(body, scope, errors, rule_id);
            scope.exit_block(saved);

            // Check catch body with error variable in scope (block-scoped)
            let saved = scope.enter_block();
            scope.define_let(catch_var);
            check_stmt(catch_body, scope, errors, rule_id);
            scope.exit_block(saved);
        }

        Stmt::Block { body, .. } => {
            let saved = scope.enter_block();
            for stmt in body {
                check_stmt(stmt, scope, errors, rule_id);
            }
            scope.exit_block(saved);
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
