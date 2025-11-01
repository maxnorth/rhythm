//! Semantic validation for Rhythm workflow AST
//!
//! This module validates workflow ASTs after parsing but before execution.
//! It checks for:
//! - Function signature correctness (argument count, types)
//! - Variable scope validity
//! - Control flow correctness (break/continue only in loops, return placement)
//! - Type checking for literal values

use anyhow::{anyhow, Result};
use serde_json::Value as JsonValue;
use std::collections::HashSet;

/// Validation error types
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// Function called with wrong number of arguments
    WrongArgumentCount {
        function: String,
        expected_min: usize,
        expected_max: usize,
        got: usize,
        line: Option<usize>,
    },
    /// Function argument has invalid type
    InvalidArgumentType {
        function: String,
        argument_index: usize,
        expected: String,
        got: String,
        line: Option<usize>,
    },
    /// Unknown function called
    UnknownFunction {
        name: String,
        line: Option<usize>,
    },
    /// Break/continue outside of loop
    ControlFlowOutsideLoop {
        statement: String,
        line: Option<usize>,
    },
    /// Variable used before declaration
    UndefinedVariable {
        name: String,
        line: Option<usize>,
    },
    /// Other validation errors
    Other {
        message: String,
        line: Option<usize>,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::WrongArgumentCount { function, expected_min, expected_max, got, line } => {
                let line_info = line.map(|l| format!(" at line {}", l)).unwrap_or_default();
                if expected_min == expected_max {
                    write!(f, "Function '{}' expects {} argument(s), got {}{}",
                           function, expected_min, got, line_info)
                } else {
                    write!(f, "Function '{}' expects {}-{} arguments, got {}{}",
                           function, expected_min, expected_max, got, line_info)
                }
            }
            ValidationError::InvalidArgumentType { function, argument_index, expected, got, line } => {
                let line_info = line.map(|l| format!(" at line {}", l)).unwrap_or_default();
                write!(f, "Function '{}' argument {} expects {}, got {}{}",
                       function, argument_index, expected, got, line_info)
            }
            ValidationError::UnknownFunction { name, line } => {
                let line_info = line.map(|l| format!(" at line {}", l)).unwrap_or_default();
                write!(f, "Unknown function '{}'{}", name, line_info)
            }
            ValidationError::ControlFlowOutsideLoop { statement, line } => {
                let line_info = line.map(|l| format!(" at line {}", l)).unwrap_or_default();
                write!(f, "'{}' statement outside of loop{}", statement, line_info)
            }
            ValidationError::UndefinedVariable { name, line } => {
                let line_info = line.map(|l| format!(" at line {}", l)).unwrap_or_default();
                write!(f, "Undefined variable '{}'{}", name, line_info)
            }
            ValidationError::Other { message, line } => {
                let line_info = line.map(|l| format!(" at line {}", l)).unwrap_or_default();
                write!(f, "{}{}", message, line_info)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Context for semantic validation
struct ValidationContext {
    /// Variables in scope at current point
    variables: Vec<HashSet<String>>,
    /// Are we currently inside a loop?
    in_loop: bool,
}

impl ValidationContext {
    fn new() -> Self {
        Self {
            variables: vec![HashSet::new()],
            in_loop: false,
        }
    }

    fn push_scope(&mut self) {
        self.variables.push(HashSet::new());
    }

    fn pop_scope(&mut self) {
        self.variables.pop();
    }

    fn declare_variable(&mut self, name: String) {
        if let Some(scope) = self.variables.last_mut() {
            scope.insert(name);
        }
    }

    fn is_variable_defined(&self, name: &str) -> bool {
        self.variables.iter().any(|scope| scope.contains(name))
    }
}

/// Validate a workflow AST
pub fn validate_workflow(ast: &JsonValue) -> Result<()> {
    let statements = ast.as_array()
        .ok_or_else(|| anyhow!("Workflow AST must be an array of statements"))?;

    let mut ctx = ValidationContext::new();

    // Add implicit variables (ctx, inputs)
    ctx.declare_variable("ctx".to_string());
    ctx.declare_variable("inputs".to_string());

    for statement in statements {
        validate_statement(statement, &mut ctx)?;
    }

    Ok(())
}

/// Validate a single statement
fn validate_statement(stmt: &JsonValue, ctx: &mut ValidationContext) -> Result<()> {
    let stmt_type = stmt.get("type")
        .and_then(|t| t.as_str())
        .ok_or_else(|| anyhow!("Statement missing 'type' field"))?;

    match stmt_type {
        "await" => {
            let expression = stmt.get("expression")
                .ok_or_else(|| anyhow!("Await statement missing 'expression' field"))?;
            validate_expression(expression, ctx)?;
        }
        "expression_statement" => {
            let expression = stmt.get("expression")
                .ok_or_else(|| anyhow!("Expression statement missing 'expression' field"))?;
            validate_expression(expression, ctx)?;
        }
        "assignment" => {
            let left = stmt.get("left")
                .ok_or_else(|| anyhow!("Assignment missing 'left' field"))?;
            let right = stmt.get("right")
                .ok_or_else(|| anyhow!("Assignment missing 'right' field"))?;

            // Validate right side first
            validate_expression(right, ctx)?;

            // Declare variable
            let var_name = left.get("name")
                .and_then(|n| n.as_str())
                .ok_or_else(|| anyhow!("Assignment left side missing 'name' field"))?;
            ctx.declare_variable(var_name.to_string());
        }
        "if" => {
            // Validate condition
            if let Some(condition) = stmt.get("condition") {
                validate_expression(condition, ctx)?;
            }

            // Validate then branch
            if let Some(then_stmts) = stmt.get("then_statements").and_then(|s| s.as_array()) {
                ctx.push_scope();
                for s in then_stmts {
                    validate_statement(s, ctx)?;
                }
                ctx.pop_scope();
            }

            // Validate else branch
            if let Some(else_stmts) = stmt.get("else_statements").and_then(|s| s.as_array()) {
                ctx.push_scope();
                for s in else_stmts {
                    validate_statement(s, ctx)?;
                }
                ctx.pop_scope();
            }
        }
        "for" => {
            // Validate iterable
            if let Some(iterable) = stmt.get("iterable") {
                validate_iterable(iterable, ctx)?;
            }

            // Enter loop context
            let was_in_loop = ctx.in_loop;
            ctx.in_loop = true;

            // Push scope for loop variable
            ctx.push_scope();

            // Declare loop variable
            if let Some(loop_var) = stmt.get("loop_variable").and_then(|v| v.as_str()) {
                ctx.declare_variable(loop_var.to_string());
            }

            // Validate body
            if let Some(body_stmts) = stmt.get("body_statements").and_then(|s| s.as_array()) {
                for s in body_stmts {
                    validate_statement(s, ctx)?;
                }
            }

            ctx.pop_scope();
            ctx.in_loop = was_in_loop;
        }
        "break" => {
            if !ctx.in_loop {
                return Err(ValidationError::ControlFlowOutsideLoop {
                    statement: "break".to_string(),
                    line: None,
                }.into());
            }
        }
        "continue" => {
            if !ctx.in_loop {
                return Err(ValidationError::ControlFlowOutsideLoop {
                    statement: "continue".to_string(),
                    line: None,
                }.into());
            }
        }
        "return" => {
            // Validate return value if present
            if let Some(value) = stmt.get("value") {
                validate_expression(value, ctx)?;
            }
        }
        _ => {
            // Unknown statement type - could warn or ignore
        }
    }

    Ok(())
}

/// Validate an expression
fn validate_expression(expr: &JsonValue, ctx: &ValidationContext) -> Result<()> {
    if let Some(expr_type) = expr.get("type").and_then(|t| t.as_str()) {
        match expr_type {
            "function_call" => validate_function_call(expr, ctx)?,
            "await" => {
                // Await expression wraps another expression
                if let Some(inner) = expr.get("expression") {
                    validate_expression(inner, ctx)?;
                }
            }
            "variable" => {
                // Check if variable is defined
                if let Some(var_name) = expr.get("name").and_then(|n| n.as_str()) {
                    if !ctx.is_variable_defined(var_name) {
                        return Err(ValidationError::UndefinedVariable {
                            name: var_name.to_string(),
                            line: None,
                        }.into());
                    }
                }
            }
            _ => {
                // Other expression types (literals, member access, etc.)
                // Recursively validate if they have nested expressions
            }
        }
    }

    // Validate nested values (arrays, objects)
    if let Some(arr) = expr.as_array() {
        for item in arr {
            validate_expression(item, ctx)?;
        }
    } else if let Some(obj) = expr.as_object() {
        for (_key, value) in obj {
            validate_expression(value, ctx)?;
        }
    }

    Ok(())
}

/// Validate a function call
fn validate_function_call(expr: &JsonValue, ctx: &ValidationContext) -> Result<()> {
    let name_parts = expr.get("name")
        .and_then(|n| n.as_array())
        .ok_or_else(|| anyhow!("Function call missing 'name' field"))?;

    let func_name = name_parts.iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>()
        .join(".");

    let args = expr.get("args")
        .and_then(|a| a.as_array())
        .ok_or_else(|| anyhow!("Function call missing 'args' field"))?;

    // Validate based on known functions
    match func_name.as_str() {
        "Task.run" => validate_task_run(args, ctx)?,
        "Task.delay" => validate_task_delay(args, ctx)?,
        _ => {
            // Unknown function - could be extensible, so just warn or allow
            // For now, we'll validate the arguments as expressions
            for arg in args {
                validate_expression(arg, ctx)?;
            }
        }
    }

    Ok(())
}

/// Validate Task.run() function call
fn validate_task_run(args: &[JsonValue], ctx: &ValidationContext) -> Result<()> {
    // Task.run(task_name: string, inputs?: object)
    if args.is_empty() || args.len() > 2 {
        return Err(ValidationError::WrongArgumentCount {
            function: "Task.run".to_string(),
            expected_min: 1,
            expected_max: 2,
            got: args.len(),
            line: None,
        }.into());
    }

    // First argument must be a string (task name)
    // Note: Could be a variable, so we check if it's a literal
    if let Some(task_name) = args[0].as_str() {
        if task_name.is_empty() {
            return Err(ValidationError::Other {
                message: "Task.run() task name cannot be empty".to_string(),
                line: None,
            }.into());
        }
    } else if !is_variable_or_expression(&args[0]) {
        return Err(ValidationError::InvalidArgumentType {
            function: "Task.run".to_string(),
            argument_index: 0,
            expected: "string (task name)".to_string(),
            got: format!("{:?}", args[0]),
            line: None,
        }.into());
    }

    // Second argument (if present) should be an object
    if args.len() == 2 {
        if !args[1].is_object() && !is_variable_or_expression(&args[1]) {
            return Err(ValidationError::InvalidArgumentType {
                function: "Task.run".to_string(),
                argument_index: 1,
                expected: "object (task inputs)".to_string(),
                got: format!("{:?}", args[1]),
                line: None,
            }.into());
        }

        // Validate input object recursively
        validate_expression(&args[1], ctx)?;
    }

    Ok(())
}

/// Validate Task.delay() function call
fn validate_task_delay(args: &[JsonValue], ctx: &ValidationContext) -> Result<()> {
    // Task.delay(duration: number)
    if args.len() != 1 {
        return Err(ValidationError::WrongArgumentCount {
            function: "Task.delay".to_string(),
            expected_min: 1,
            expected_max: 1,
            got: args.len(),
            line: None,
        }.into());
    }

    // Argument must be a number
    if !args[0].is_number() && !is_variable_or_expression(&args[0]) {
        return Err(ValidationError::InvalidArgumentType {
            function: "Task.delay".to_string(),
            argument_index: 0,
            expected: "number (milliseconds)".to_string(),
            got: format!("{:?}", args[0]),
            line: None,
        }.into());
    }

    // If it's a literal number, check it's positive
    if let Some(duration) = args[0].as_i64() {
        if duration < 0 {
            return Err(ValidationError::Other {
                message: "Task.delay() duration must be non-negative".to_string(),
                line: None,
            }.into());
        }
    }

    // Validate the expression
    validate_expression(&args[0], ctx)?;

    Ok(())
}

/// Validate an iterable specification (for loop)
fn validate_iterable(iterable: &JsonValue, ctx: &ValidationContext) -> Result<()> {
    if let Some(iter_type) = iterable.get("type").and_then(|t| t.as_str()) {
        match iter_type {
            "array" => {
                // Inline array
                if let Some(value) = iterable.get("value") {
                    validate_expression(value, ctx)?;
                }
            }
            "variable" => {
                // Variable reference
                if let Some(value) = iterable.get("value") {
                    validate_expression(value, ctx)?;
                }
            }
            "member_access" => {
                // Member access like inputs.items
                validate_expression(iterable, ctx)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Check if a value is a variable reference or complex expression
fn is_variable_or_expression(value: &JsonValue) -> bool {
    if let Some(obj) = value.as_object() {
        // Check for variable, member access, function call, etc.
        if let Some(type_str) = obj.get("type").and_then(|t| t.as_str()) {
            matches!(type_str, "variable" | "member_access" | "function_call" | "await")
        } else {
            false
        }
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::parse_workflow;

    #[test]
    fn test_validate_task_run_valid() {
        let source = r#"
workflow(ctx, inputs) {
    await Task.run("my-task", {})
}
        "#;
        let ast = parse_workflow(source).unwrap();
        let ast_json = serde_json::to_value(&ast).unwrap();

        assert!(validate_workflow(&ast_json).is_ok());
    }

    #[test]
    fn test_validate_task_run_no_args() {
        // This test validates that Task.run requires at least 1 argument
        // We need to construct invalid AST manually since the parser won't parse invalid syntax
        let ast_json = serde_json::json!([
            {
                "type": "expression_statement",
                "expression": {
                    "type": "function_call",
                    "name": ["Task", "run"],
                    "args": []
                }
            }
        ]);

        let result = validate_workflow(&ast_json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("expects 1-2 arguments"));
    }

    #[test]
    fn test_validate_task_delay_valid() {
        let source = r#"
workflow(ctx, inputs) {
    await Task.delay(1000)
}
        "#;
        let ast = parse_workflow(source).unwrap();
        let ast_json = serde_json::to_value(&ast).unwrap();

        assert!(validate_workflow(&ast_json).is_ok());
    }

    #[test]
    fn test_validate_task_delay_wrong_type() {
        // Parser accepts strings as args, so we need to test that validator catches wrong type
        let source = r#"
workflow(ctx, inputs) {
    await Task.delay("not a number")
}
        "#;
        let ast = parse_workflow(source).unwrap();
        let ast_json = serde_json::to_value(&ast).unwrap();

        let result = validate_workflow(&ast_json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("expects number"));
    }

    #[test]
    fn test_validate_break_outside_loop() {
        let source = r#"
workflow(ctx, inputs) {
    break
}
        "#;
        let ast = parse_workflow(source).unwrap();
        let ast_json = serde_json::to_value(&ast).unwrap();

        let result = validate_workflow(&ast_json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("outside of loop"));
    }

    #[test]
    fn test_validate_break_inside_loop() {
        let source = r#"
workflow(ctx, inputs) {
    for (let item in [1, 2, 3]) {
        break
    }
}
        "#;
        let ast = parse_workflow(source).unwrap();
        let ast_json = serde_json::to_value(&ast).unwrap();

        assert!(validate_workflow(&ast_json).is_ok());
    }

    #[test]
    fn test_validate_variable_scope() {
        let source = r#"
workflow(ctx, inputs) {
    let x = Task.run("task1", {})
    await Task.run("task2", { value: x })
}
        "#;
        let ast = parse_workflow(source).unwrap();
        let ast_json = serde_json::to_value(&ast).unwrap();

        assert!(validate_workflow(&ast_json).is_ok());
    }

    #[test]
    fn test_validate_undefined_variable() {
        let source = r#"
workflow(ctx, inputs) {
    await Task.run("task1", { value: undefined_var })
}
        "#;
        let ast = parse_workflow(source).unwrap();
        let ast_json = serde_json::to_value(&ast).unwrap();

        let result = validate_workflow(&ast_json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Undefined variable"));
    }

    #[test]
    fn test_validate_continue_outside_loop() {
        let source = r#"
workflow(ctx, inputs) {
    continue
}
        "#;
        let ast = parse_workflow(source).unwrap();
        let ast_json = serde_json::to_value(&ast).unwrap();

        let result = validate_workflow(&ast_json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("outside of loop"));
    }

    #[test]
    fn test_validate_break_in_if_outside_loop() {
        let source = r#"
workflow(ctx, inputs) {
    if (inputs.flag == true) {
        break
    }
}
        "#;
        let ast = parse_workflow(source).unwrap();
        let ast_json = serde_json::to_value(&ast).unwrap();

        let result = validate_workflow(&ast_json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("outside of loop"));
    }
}
