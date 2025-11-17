use pest::Parser;
use pest_derive::Parser;
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;

#[derive(Parser)]
#[grammar = "interpreter/workflow.pest"]
struct WorkflowParser;

/// Parser context for tracking variable scopes during parsing
struct ParserContext {
    /// Current scope depth (0 = global)
    scope_depth: usize,
    /// Stack of scopes, each containing variable name -> depth where defined
    symbol_table: Vec<HashMap<String, usize>>,
}

impl ParserContext {
    fn new() -> Self {
        Self {
            scope_depth: 0,
            symbol_table: vec![HashMap::new()], // Start with global scope
        }
    }

    /// Enter a new scope (e.g., for loop, if block)
    fn enter_scope(&mut self) {
        self.scope_depth += 1;
        self.symbol_table.push(HashMap::new());
    }

    /// Exit current scope
    fn exit_scope(&mut self) {
        self.symbol_table.pop();
        self.scope_depth = self.scope_depth.saturating_sub(1);
    }

    /// Declare a variable in the current scope
    fn declare_variable(&mut self, name: String) {
        if let Some(current_scope) = self.symbol_table.last_mut() {
            current_scope.insert(name, self.scope_depth);
        }
    }

    /// Look up a variable and return the depth where it's defined
    /// Walks from current scope to global scope
    fn lookup_variable(&self, name: &str) -> Option<usize> {
        // Walk from current depth down to 0
        for depth in (0..=self.scope_depth).rev() {
            if let Some(scope) = self.symbol_table.get(depth) {
                if scope.contains_key(name) {
                    return Some(depth);
                }
            }
        }
        None
    }

    fn current_depth(&self) -> usize {
        self.scope_depth
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    UnexpectedToken { line: usize, message: String },
    InvalidJson { line: usize, message: String },
    InvalidStatement { line: usize, message: String },
    UnknownFunction { line: usize, function: String },
    PestError(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnexpectedToken { line, message } => {
                write!(f, "Parse error on line {}: {}", line, message)
            }
            ParseError::InvalidJson { line, message } => {
                write!(f, "Invalid JSON on line {}: {}", line, message)
            }
            ParseError::InvalidStatement { line, message } => {
                write!(f, "Invalid statement on line {}: {}", line, message)
            }
            ParseError::UnknownFunction { line, function } => {
                write!(f, "Unknown function '{}' on line {}", function, line)
            }
            ParseError::PestError(msg) => {
                write!(f, "Parse error: {}", msg)
            }
        }
    }
}

impl std::error::Error for ParseError {}

impl From<pest::error::Error<Rule>> for ParseError {
    fn from(err: pest::error::Error<Rule>) -> Self {
        ParseError::PestError(err.to_string())
    }
}

/// Parse a .flow workflow script into a JSON array of steps
///
/// Input example:
/// ```
/// Task.run("do-something", { "hey": "hello" })
/// await Task.run("wait-for-result", {})
/// Task.run("do-another")
/// ```
///
/// Output:
/// ```json
/// [
///   {
///     "type": "expression_statement",
///     "expression": { "type": "function_call", "name": ["Task", "run"], "args": ["do-something", { "hey": "hello" }] }
///   },
///   {
///     "type": "await",
///     "expression": { "type": "function_call", "name": ["Task", "delay"], "args": [10] }
///   },
///   {
///     "type": "expression_statement",
///     "expression": { "type": "function_call", "name": ["Task", "run"], "args": ["do-another", {}] }
///   }
/// ]
/// ```
pub fn parse_workflow(source: &str) -> Result<Vec<JsonValue>, ParseError> {
    let mut ctx = ParserContext::new();
    // Check for semicolons and provide helpful error message
    if let Some(pos) = source.find(';') {
        let line = source[..pos].lines().count();
        return Err(ParseError::UnexpectedToken {
            line,
            message: "Semicolons are not allowed in workflow syntax. Each statement should be on its own line.".to_string(),
        });
    }

    // Check for # comments (but not inside strings) and provide helpful error message
    let mut in_string = false;
    let mut escape_next = false;
    let mut quote_char: Option<char> = None;

    for (i, ch) in source.chars().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }

        if ch == '\\' && in_string {
            escape_next = true;
            continue;
        }

        if !in_string && (ch == '"' || ch == '\'') {
            in_string = true;
            quote_char = Some(ch);
        } else if in_string && Some(ch) == quote_char {
            in_string = false;
            quote_char = None;
        } else if !in_string && ch == '#' {
            let line = source[..i].lines().count();
            return Err(ParseError::UnexpectedToken {
                line,
                message: "Use '//' for comments instead of '#'.".to_string(),
            });
        }
    }

    let mut pairs = WorkflowParser::parse(Rule::workflow, source)?;

    // Extract the workflow_function
    let workflow_func = pairs.next()
        .ok_or_else(|| ParseError::PestError("expected workflow function".to_string()))?;

    if workflow_func.as_rule() != Rule::workflow_function {
        return Err(ParseError::PestError("expected workflow(ctx, inputs) { ... }".to_string()));
    }

    let mut steps = Vec::new();
    let mut last_statement_line: Option<usize> = None;

    // Parse the workflow function inner elements
    let mut inner = workflow_func.into_inner();

    // First two are identifiers (ctx, inputs) - we'll validate but not use them yet
    let ctx_param = inner.next()
        .ok_or_else(|| ParseError::PestError("expected ctx parameter".to_string()))?;
    let inputs_param = inner.next()
        .ok_or_else(|| ParseError::PestError("expected inputs parameter".to_string()))?;

    // Store parameter names for validation (could use later for better error messages)
    let _ctx_name = ctx_param.as_str();
    let _inputs_name = inputs_param.as_str();

    // Rest are statements
    for statement in inner {
        // The statement is a wrapper - get the inner content
        let statement_line = statement.as_span().start_pos().line_col().0;

        // Check if this statement is on the same line as the previous one
        if let Some(prev_line) = last_statement_line {
            if prev_line == statement_line {
                return Err(ParseError::UnexpectedToken {
                    line: statement_line,
                    message: "Multiple statements on the same line are not allowed. Each statement must be on its own line.".to_string(),
                });
            }
        }

        last_statement_line = Some(statement_line);

        // Unwrap the statement to get the actual content
        for inner_statement in statement.into_inner() {
            let line = inner_statement.as_span().start_pos().line_col().0;
            match inner_statement.as_rule() {
                Rule::assignment => {
                    steps.push(parse_assignment(inner_statement, &mut ctx)?);
                }
                Rule::await_statement => {
                    steps.push(parse_await_statement(inner_statement, &mut ctx)?);
                }
                Rule::expression_statement => {
                    let func_call = inner_statement.into_inner().next()
                        .ok_or_else(|| ParseError::PestError("expression_statement missing function_call".to_string()))?;
                    steps.push(parse_expression_statement(func_call, line, &ctx)?);
                }
                Rule::if_statement => {
                    steps.push(parse_if_statement(inner_statement, &mut ctx)?);
                }
                Rule::for_loop => {
                    steps.push(parse_for_loop(inner_statement, &mut ctx)?);
                }
                Rule::break_statement => {
                    steps.push(json!({
                        "type": "break"
                    }));
                }
                Rule::continue_statement => {
                    steps.push(json!({
                        "type": "continue"
                    }));
                }
                Rule::return_statement => {
                    steps.push(parse_return_statement(inner_statement, &mut ctx)?);
                }
                _ => {}
            }
        }
    }

    Ok(steps)
}

fn parse_return_statement(pair: pest::iterators::Pair<Rule>, ctx: &ParserContext) -> Result<JsonValue, ParseError> {
    let line = pair.as_span().start_pos().line_col().0;
    let mut inner = pair.into_inner();

    // Check if there's a return value
    let return_value = if let Some(value_pair) = inner.next() {
        parse_json_value(value_pair, line, ctx)?
    } else {
        JsonValue::Null
    };

    Ok(json!({
        "type": "return",
        "value": return_value
    }))
}

/// Parse an expression (anything that evaluates to a value)
fn parse_expression(pair: pest::iterators::Pair<Rule>, line: usize, ctx: &ParserContext) -> Result<JsonValue, ParseError> {
    let inner = pair.into_inner().next()
        .ok_or_else(|| ParseError::InvalidJson {
            line,
            message: "Empty expression".to_string(),
        })?;

    match inner.as_rule() {
        Rule::await_expression => parse_await_expression(inner, line, ctx),
        Rule::function_call => parse_function_call(inner, line, ctx),
        Rule::member_access => parse_member_access(inner, line, ctx),
        Rule::json_value => parse_json_value(inner, line, ctx),
        _ => Err(ParseError::InvalidJson {
            line,
            message: format!("Unknown expression type: {:?}", inner.as_rule()),
        }),
    }
}

/// Parse await expression: await function_call or await member_access
fn parse_await_expression(pair: pest::iterators::Pair<Rule>, line: usize, ctx: &ParserContext) -> Result<JsonValue, ParseError> {
    let mut inner = pair.into_inner();
    let expr = inner.next()
        .ok_or_else(|| ParseError::InvalidJson {
            line,
            message: "await expression missing expression".to_string(),
        })?;

    let inner_expr = match expr.as_rule() {
        Rule::function_call => parse_function_call(expr, line, ctx)?,
        Rule::member_access => parse_member_access(expr, line, ctx)?,
        Rule::json_value => parse_json_value(expr, line, ctx)?,
        _ => return Err(ParseError::InvalidJson {
            line,
            message: format!("Invalid expression after await: {:?}", expr.as_rule()),
        }),
    };

    Ok(json!({
        "type": "await",
        "expression": inner_expr
    }))
}

/// Parse function call: functionName(arg1, arg2, ...) or Namespace.functionName(arg1, ...)
fn parse_function_call(pair: pest::iterators::Pair<Rule>, line: usize, ctx: &ParserContext) -> Result<JsonValue, ParseError> {
    let mut inner = pair.into_inner();

    // Collect namespace path (e.g., ["Task", "run"] for Task.run())
    let mut name_parts = Vec::new();
    let mut args = Vec::new();

    // Collect all identifiers and expressions
    for next in inner {
        if next.as_rule() == Rule::identifier {
            name_parts.push(next.as_str().to_string());
        } else {
            // This is an argument expression
            let arg_line = next.as_span().start_pos().line_col().0;
            let arg = parse_expression(next, arg_line, ctx)?;
            args.push(arg);
        }
    }

    if name_parts.is_empty() {
        return Err(ParseError::InvalidJson {
            line,
            message: "Function call missing name".to_string(),
        });
    }

    Ok(json!({
        "type": "function_call",
        "name": name_parts,
        "args": args
    }))
}

fn parse_assignment(pair: pest::iterators::Pair<Rule>, ctx: &mut ParserContext) -> Result<JsonValue, ParseError> {
    let mut inner = pair.into_inner();

    // Get variable name (identifier) - this is the "left" side
    let var_name_pair = inner.next()
        .ok_or_else(|| ParseError::PestError("assignment requires variable name".to_string()))?;
    let var_name = var_name_pair.as_str().to_string();

    // Declare the variable in current scope
    ctx.declare_variable(var_name.clone());

    // Get the right-hand side expression
    let rhs_pair = inner.next()
        .ok_or_else(|| ParseError::PestError("assignment requires right-hand side expression".to_string()))?;

    let line = rhs_pair.as_span().start_pos().line_col().0;

    // Parse the right side as an expression
    let right_expr = parse_expression(rhs_pair, line, ctx)?;

    // Create assignment with left/right structure
    Ok(json!({
        "type": "assignment",
        "left": {
            "type": "variable",
            "name": var_name,
            "depth": ctx.current_depth()
        },
        "right": right_expr
    }))
}

fn parse_await_statement(pair: pest::iterators::Pair<Rule>, ctx: &mut ParserContext) -> Result<JsonValue, ParseError> {
    let mut inner = pair.into_inner();

    // Get the expression to await
    let expr_pair = inner.next()
        .ok_or_else(|| ParseError::PestError("await requires an expression".to_string()))?;

    let line = expr_pair.as_span().start_pos().line_col().0;

    // Parse the expression
    let expression = parse_expression(expr_pair, line, ctx)?;

    // Wrap in await
    Ok(json!({
        "type": "await",
        "expression": expression
    }))
}

fn parse_expression_statement(pair: pest::iterators::Pair<Rule>, line: usize, ctx: &ParserContext) -> Result<JsonValue, ParseError> {
    // Expression statement is just a function call used for side effects
    let func_call = parse_function_call(pair, line, ctx)?;

    Ok(json!({
        "type": "expression_statement",
        "expression": func_call
    }))
}

/// Helper to parse a statement (used in for loops, if statements, etc.)
fn parse_single_statement(inner_statement: pest::iterators::Pair<Rule>, ctx: &mut ParserContext) -> Result<JsonValue, ParseError> {
    let line = inner_statement.as_span().start_pos().line_col().0;
    match inner_statement.as_rule() {
        Rule::assignment => parse_assignment(inner_statement, ctx),
        Rule::await_statement => parse_await_statement(inner_statement, ctx),
        Rule::expression_statement => {
            let func_call = inner_statement.into_inner().next()
                .ok_or_else(|| ParseError::PestError("expression_statement missing function_call".to_string()))?;
            parse_expression_statement(func_call, line, ctx)
        }
        Rule::if_statement => parse_if_statement(inner_statement, ctx),
        Rule::for_loop => parse_for_loop(inner_statement, ctx),
        Rule::break_statement => Ok(json!({"type": "break"})),
        Rule::continue_statement => Ok(json!({"type": "continue"})),
        Rule::return_statement => parse_return_statement(inner_statement, ctx),
        _ => Err(ParseError::PestError(format!("Unknown statement type: {:?}", inner_statement.as_rule()))),
    }
}

fn parse_string(pair: pest::iterators::Pair<Rule>) -> Result<String, ParseError> {
    // The string is atomic (@), so we get the whole thing including quotes
    // We need to strip the first and last character (the quotes) and handle escape sequences
    let s = pair.as_str();

    if s.len() < 2 {
        return Ok(String::new());
    }

    // Strip quotes
    let content = &s[1..s.len()-1];

    // Handle escape sequences
    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // Handle escape sequence
            if let Some(next_ch) = chars.next() {
                match next_ch {
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    'r' => result.push('\r'),
                    '\\' => result.push('\\'),
                    '"' => result.push('"'),
                    '\'' => result.push('\''),
                    // For any other escape, just include the character (e.g., \u -> u)
                    _ => result.push(next_ch),
                }
            } else {
                // Trailing backslash - just add it
                result.push('\\');
            }
        } else if ch == '$' && chars.peek() == Some(&'$') {
            // Escape sequence: $$ -> $ (literal dollar sign for strings like "$$99.99" -> "$99.99")
            chars.next(); // consume the second $
            result.push('$');
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}

fn parse_json_object(pair: pest::iterators::Pair<Rule>, line: usize, ctx: &ParserContext) -> Result<JsonValue, ParseError> {
    let mut obj = serde_json::Map::new();

    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::json_pair {
            let mut pair_inner = inner.into_inner();

            // Get key (string or identifier)
            let key_pair = pair_inner.next()
                .ok_or_else(|| ParseError::InvalidJson {
                    line,
                    message: "JSON pair missing key".to_string(),
                })?;

            // Key can be either a quoted string or an unquoted identifier
            let key = match key_pair.as_rule() {
                Rule::string => parse_string(key_pair)?,
                Rule::identifier => key_pair.as_str().to_string(),
                _ => return Err(ParseError::InvalidJson {
                    line,
                    message: format!("Invalid JSON key type: {:?}", key_pair.as_rule()),
                }),
            };

            // Get value (json_value)
            let value_pair = pair_inner.next()
                .ok_or_else(|| ParseError::InvalidJson {
                    line,
                    message: "JSON pair missing value".to_string(),
                })?;
            let value = parse_json_value(value_pair, line, ctx)?;

            obj.insert(key, value);
        }
    }

    Ok(JsonValue::Object(obj))
}

/// Parse member access into a structured format
/// Examples:
///   inputs.user -> {"base": "inputs", "path": [{"type": "dot", "value": "user"}]}
///   data[0] -> {"base": "data", "path": [{"type": "index", "value": 0}]}
///   items[0].name -> {"base": "items", "path": [{"type": "index", "value": 0}, {"type": "dot", "value": "name"}]}
///   obj["key"] -> {"base": "obj", "path": [{"type": "bracket", "value": "key"}]}
fn parse_member_access(pair: pest::iterators::Pair<Rule>, line: usize, ctx: &ParserContext) -> Result<JsonValue, ParseError> {
    let mut inner = pair.into_inner();

    // First part is always an identifier (the base)
    let base = inner.next()
        .ok_or_else(|| ParseError::InvalidJson {
            line,
            message: "Member access missing base identifier".to_string(),
        })?
        .as_str()
        .to_string();

    let mut path = Vec::new();

    // Process property accessors
    for accessor in inner {
        match accessor.as_rule() {
            Rule::property_accessor => {
                let mut accessor_inner = accessor.into_inner();
                let accessor_part = accessor_inner.next()
                    .ok_or_else(|| ParseError::InvalidJson {
                        line,
                        message: "Empty property accessor".to_string(),
                    })?;

                match accessor_part.as_rule() {
                    Rule::identifier => {
                        // Dot notation: .fieldName
                        path.push(json!({
                            "type": "dot",
                            "value": accessor_part.as_str()
                        }));
                    }
                    Rule::bracket_accessor => {
                        // Bracket notation: [0] or ["key"] or [varName]
                        let mut bracket_inner = accessor_part.into_inner();
                        let index_or_key = bracket_inner.next()
                            .ok_or_else(|| ParseError::InvalidJson {
                                line,
                                message: "Empty bracket accessor".to_string(),
                            })?;

                        match index_or_key.as_rule() {
                            Rule::integer => {
                                // Numeric index: [0]
                                let index = index_or_key.as_str().parse::<i64>()
                                    .map_err(|_| ParseError::InvalidJson {
                                        line,
                                        message: format!("Invalid array index: {}", index_or_key.as_str()),
                                    })?;
                                path.push(json!({
                                    "type": "index",
                                    "value": index
                                }));
                            }
                            Rule::string => {
                                // String key: ["key"]
                                let key = parse_string(index_or_key)?;
                                path.push(json!({
                                    "type": "bracket",
                                    "value": key
                                }));
                            }
                            Rule::identifier => {
                                // Variable reference: [varName]
                                // Store with depth for runtime resolution
                                let var_name = index_or_key.as_str();
                                path.push(json!({
                                    "type": "bracket_var",
                                    "value": var_name,
                                    "depth": ctx.scope_depth
                                }));
                            }
                            _ => {
                                return Err(ParseError::InvalidJson {
                                    line,
                                    message: format!("Invalid bracket accessor content: {:?}", index_or_key.as_rule()),
                                });
                            }
                        }
                    }
                    _ => {
                        return Err(ParseError::InvalidJson {
                            line,
                            message: format!("Invalid property accessor: {:?}", accessor_part.as_rule()),
                        });
                    }
                }
            }
            _ => {
                return Err(ParseError::InvalidJson {
                    line,
                    message: format!("Unexpected rule in member access: {:?}", accessor.as_rule()),
                });
            }
        }
    }

    Ok(json!({
        "base": base,
        "path": path
    }))
}

fn parse_json_value(pair: pest::iterators::Pair<Rule>, line: usize, ctx: &ParserContext) -> Result<JsonValue, ParseError> {
    let inner = pair.into_inner().next()
        .ok_or_else(|| ParseError::InvalidJson {
            line,
            message: "Empty JSON value".to_string(),
        })?;

    match inner.as_rule() {
        Rule::json_object => parse_json_object(inner, line, ctx),
        Rule::json_array => parse_json_array(inner, line, ctx),
        Rule::string => Ok(JsonValue::String(parse_string(inner)?)),
        Rule::number => {
            let num_str = inner.as_str();

            // Handle hex numbers (0x1234)
            if num_str.starts_with("0x") || num_str.starts_with("0X") ||
               num_str.starts_with("-0x") || num_str.starts_with("-0X") {
                let (sign, hex_part) = if num_str.starts_with('-') {
                    (-1i64, &num_str[3..])
                } else {
                    (1i64, &num_str[2..])
                };
                let hex_clean = hex_part.replace('_', "");
                if let Ok(val) = i64::from_str_radix(&hex_clean, 16) {
                    return Ok(JsonValue::Number((sign * val).into()));
                } else {
                    return Err(ParseError::InvalidJson {
                        line,
                        message: format!("Invalid hex number: {}", num_str),
                    });
                }
            }

            // Handle binary numbers (0b1010)
            if num_str.starts_with("0b") || num_str.starts_with("0B") ||
               num_str.starts_with("-0b") || num_str.starts_with("-0B") {
                let (sign, bin_part) = if num_str.starts_with('-') {
                    (-1i64, &num_str[3..])
                } else {
                    (1i64, &num_str[2..])
                };
                let bin_clean = bin_part.replace('_', "");
                if let Ok(val) = i64::from_str_radix(&bin_clean, 2) {
                    return Ok(JsonValue::Number((sign * val).into()));
                } else {
                    return Err(ParseError::InvalidJson {
                        line,
                        message: format!("Invalid binary number: {}", num_str),
                    });
                }
            }

            // Handle regular numbers (with underscores)
            let clean_str = num_str.replace('_', "");

            // Try parsing as integer first, then float
            if let Ok(i) = clean_str.parse::<i64>() {
                Ok(JsonValue::Number(i.into()))
            } else if let Ok(f) = clean_str.parse::<f64>() {
                Ok(serde_json::Number::from_f64(f)
                    .map(JsonValue::Number)
                    .unwrap_or(JsonValue::Null))
            } else {
                Err(ParseError::InvalidJson {
                    line,
                    message: format!("Invalid number: {}", num_str),
                })
            }
        }
        Rule::boolean => {
            let bool_val = inner.as_str() == "true";
            Ok(JsonValue::Bool(bool_val))
        }
        Rule::null => Ok(JsonValue::Null),
        Rule::member_access => {
            // Parse member access into structured format
            // Returns {"base": "inputs", "path": [{"type": "dot", "value": "user"}, ...]}
            parse_member_access(inner, line, ctx)
        }
        Rule::identifier => {
            // This is a variable reference
            // In .flow files, users write: Task.run("foo", { "key": myVar })
            // Parser outputs JSON: { "key": {"type": "variable", "name": "myVar", "depth": 0} }
            let var_name = inner.as_str();

            let depth = ctx.lookup_variable(var_name).unwrap_or(0);
            Ok(json!({
                "type": "variable",
                "name": var_name,
                "depth": depth
            }))
        }
        _ => Err(ParseError::InvalidJson {
            line,
            message: format!("Unexpected JSON value type: {:?}", inner.as_rule()),
        }),
    }
}

fn parse_json_array(pair: pest::iterators::Pair<Rule>, line: usize, ctx: &ParserContext) -> Result<JsonValue, ParseError> {
    let mut arr = Vec::new();

    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::json_value {
            arr.push(parse_json_value(inner, line, ctx)?);
        }
    }

    Ok(JsonValue::Array(arr))
}

fn parse_for_loop(pair: pest::iterators::Pair<Rule>, ctx: &mut ParserContext) -> Result<JsonValue, ParseError> {
    let line = pair.as_span().start_pos().line_col().0;
    let mut inner = pair.into_inner();

    // Get the loop variable name
    let loop_var_pair = inner.next()
        .ok_or_else(|| ParseError::UnexpectedToken {
            line,
            message: "for loop requires a variable name".to_string(),
        })?;
    let loop_variable = loop_var_pair.as_str().to_string();

    // Get the iterable (what we're looping over)
    let iterable_pair = inner.next()
        .ok_or_else(|| ParseError::UnexpectedToken {
            line,
            message: "for loop requires an iterable".to_string(),
        })?;

    let iterable = parse_for_iterable(iterable_pair, line, ctx)?;

    // Enter a new scope for the loop body
    ctx.enter_scope();

    // Declare the loop variable in the new scope
    ctx.declare_variable(loop_variable.clone());

    // Collect loop body statements
    let mut body_statements = Vec::new();
    for item in inner {
        if item.as_rule() == Rule::statement {
            for inner_statement in item.into_inner() {
                let stmt = parse_single_statement(inner_statement, ctx)?;
                body_statements.push(stmt);
            }
        }
    }

    // Exit the loop scope
    ctx.exit_scope();

    Ok(json!({
        "type": "for",
        "loop_variable": loop_variable,
        "iterable": iterable,
        "body_statements": body_statements,
    }))
}

fn parse_for_iterable(pair: pest::iterators::Pair<Rule>, line: usize, ctx: &ParserContext) -> Result<JsonValue, ParseError> {
    let inner = pair.into_inner().next()
        .ok_or_else(|| ParseError::InvalidJson {
            line,
            message: "Empty for loop iterable".to_string(),
        })?;

    match inner.as_rule() {
        Rule::member_access => {
            // Member access like inputs.items or result.data
            // Parse into structured format and wrap in type envelope
            let parsed = parse_member_access(inner, line, ctx)?;
            Ok(json!({
                "type": "member_access",
                "value": parsed
            }))
        }
        Rule::identifier => {
            let var_name = inner.as_str();
            let depth = ctx.lookup_variable(var_name).unwrap_or(0);

            Ok(json!({
                "type": "variable",
                "value": {
                    "var": var_name,
                    "depth": depth
                }
            }))
        }
        Rule::json_array => {
            // Inline array
            let array_value = parse_json_array(inner, line, ctx)?;
            Ok(json!({
                "type": "array",
                "value": array_value
            }))
        }
        _ => Err(ParseError::InvalidJson {
            line,
            message: format!("Invalid for loop iterable type: {:?}", inner.as_rule()),
        })
    }
}

fn parse_if_statement(pair: pest::iterators::Pair<Rule>, ctx: &mut ParserContext) -> Result<JsonValue, ParseError> {
    let line = pair.as_span().start_pos().line_col().0;
    let mut inner = pair.into_inner();

    // Get the condition
    let condition_pair = inner.next()
        .ok_or_else(|| ParseError::UnexpectedToken {
            line,
            message: "if statement requires a condition".to_string(),
        })?;

    let condition = parse_condition(condition_pair, line, ctx)?;

    // Collect then-branch statements
    let mut then_statements = Vec::new();
    let mut else_statements: Option<Vec<JsonValue>> = None;

    // Process remaining pairs (statements and optional else clause)
    for item in inner {
        match item.as_rule() {
            Rule::statement => {
                // Parse statement for then branch
                for inner_statement in item.into_inner() {
                    let stmt = parse_single_statement(inner_statement, ctx)?;
                    then_statements.push(stmt);
                }
            }
            Rule::else_clause => {
                // Parse else clause
                let mut else_stmts = Vec::new();
                for else_item in item.into_inner() {
                    if else_item.as_rule() == Rule::statement {
                        for inner_statement in else_item.into_inner() {
                            let stmt = parse_single_statement(inner_statement, ctx)?;
                            else_stmts.push(stmt);
                        }
                    }
                }
                else_statements = Some(else_stmts);
            }
            _ => {}
        }
    }

    let mut result = serde_json::json!({
        "type": "if",
        "condition": condition,
        "then_statements": then_statements,
    });

    if let Some(else_stmts) = else_statements {
        result["else_statements"] = JsonValue::Array(else_stmts);
    }

    Ok(result)
}

fn parse_condition(pair: pest::iterators::Pair<Rule>, line: usize, ctx: &ParserContext) -> Result<JsonValue, ParseError> {
    // Condition is just an or_expr
    let or_expr = pair.into_inner().next()
        .ok_or_else(|| ParseError::InvalidJson {
            line,
            message: "Empty condition".to_string(),
        })?;

    parse_or_expr(or_expr, line, ctx)
}

fn parse_or_expr(pair: pest::iterators::Pair<Rule>, line: usize, ctx: &ParserContext) -> Result<JsonValue, ParseError> {
    let mut parts: Vec<JsonValue> = Vec::new();

    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::and_expr {
            parts.push(parse_and_expr(inner, line, ctx)?);
        }
    }

    if parts.len() == 1 {
        Ok(parts.into_iter().next().unwrap())
    } else {
        Ok(json!({
            "type": "or",
            "operands": parts
        }))
    }
}

fn parse_and_expr(pair: pest::iterators::Pair<Rule>, line: usize, ctx: &ParserContext) -> Result<JsonValue, ParseError> {
    let mut parts: Vec<JsonValue> = Vec::new();

    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::comparison {
            parts.push(parse_comparison(inner, line, ctx)?);
        }
    }

    if parts.len() == 1 {
        Ok(parts.into_iter().next().unwrap())
    } else {
        Ok(json!({
            "type": "and",
            "operands": parts
        }))
    }
}

fn parse_comparison(pair: pest::iterators::Pair<Rule>, line: usize, ctx: &ParserContext) -> Result<JsonValue, ParseError> {
    let mut inner = pair.into_inner();

    // Get first element
    let first_pair = inner.next()
        .ok_or_else(|| ParseError::InvalidJson {
            line,
            message: "Comparison missing value".to_string(),
        })?;

    // Check if it's a parenthesized condition
    if first_pair.as_rule() == Rule::condition {
        // This is a parenthesized condition: (condition)
        return parse_condition(first_pair, line, ctx);
    }

    // Otherwise, it's a comparison_value
    let left = parse_comparison_value(first_pair, line, ctx)?;

    // Check if there's an operator
    if let Some(op_pair) = inner.next() {
        let operator = op_pair.as_str().to_string();

        // Get right value
        let right_pair = inner.next()
            .ok_or_else(|| ParseError::InvalidJson {
                line,
                message: "Comparison missing right value".to_string(),
            })?;
        let right = parse_comparison_value(right_pair, line, ctx)?;

        Ok(json!({
            "type": "comparison",
            "operator": operator,
            "left": left,
            "right": right
        }))
    } else {
        // No operator, just a value (for boolean expressions)
        Ok(left)
    }
}

fn parse_comparison_value(pair: pest::iterators::Pair<Rule>, line: usize, ctx: &ParserContext) -> Result<JsonValue, ParseError> {
    let inner = pair.into_inner().next()
        .ok_or_else(|| ParseError::InvalidJson {
            line,
            message: "Empty comparison value".to_string(),
        })?;

    match inner.as_rule() {
        Rule::member_access => {
            // Member access like inputs.userId or result.status
            // Parse into structured format
            parse_member_access(inner, line, ctx)
        }
        Rule::identifier => {
            let var_name = inner.as_str();
            let depth = ctx.lookup_variable(var_name).unwrap_or(0);

            Ok(json!({
                "var": var_name,
                "depth": depth
            }))
        }
        Rule::string => {
            Ok(JsonValue::String(parse_string(inner)?))
        }
        Rule::number => {
            // Parse number directly
            let num_str = inner.as_str();

            // Handle hex numbers
            if num_str.starts_with("0x") || num_str.starts_with("0X") ||
               num_str.starts_with("-0x") || num_str.starts_with("-0X") {
                let (sign, hex_part) = if num_str.starts_with('-') {
                    (-1i64, &num_str[3..])
                } else {
                    (1i64, &num_str[2..])
                };
                let hex_clean = hex_part.replace('_', "");
                if let Ok(val) = i64::from_str_radix(&hex_clean, 16) {
                    return Ok(JsonValue::Number((sign * val).into()));
                }
            }

            // Handle binary numbers
            if num_str.starts_with("0b") || num_str.starts_with("0B") ||
               num_str.starts_with("-0b") || num_str.starts_with("-0B") {
                let (sign, bin_part) = if num_str.starts_with('-') {
                    (-1i64, &num_str[3..])
                } else {
                    (1i64, &num_str[2..])
                };
                let bin_clean = bin_part.replace('_', "");
                if let Ok(val) = i64::from_str_radix(&bin_clean, 2) {
                    return Ok(JsonValue::Number((sign * val).into()));
                }
            }

            // Handle regular numbers (with underscores)
            let clean_str = num_str.replace('_', "");

            // Try parsing as integer first, then float
            if let Ok(i) = clean_str.parse::<i64>() {
                Ok(JsonValue::Number(i.into()))
            } else if let Ok(f) = clean_str.parse::<f64>() {
                Ok(serde_json::Number::from_f64(f)
                    .map(JsonValue::Number)
                    .unwrap_or(JsonValue::Null))
            } else {
                Err(ParseError::InvalidJson {
                    line,
                    message: format!("Invalid number: {}", num_str),
                })
            }
        }
        Rule::boolean => {
            Ok(JsonValue::Bool(inner.as_str() == "true"))
        }
        Rule::null => {
            Ok(JsonValue::Null)
        }
        _ => Err(ParseError::InvalidJson {
            line,
            message: format!("Invalid comparison value type: {:?}", inner.as_rule()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_workflow() {
        let source = r#"
workflow(ctx, inputs) {
  Task.run("do-something", { "hey": "hello" })
  await Task.run("await-task", { "value": 10 })
  Task.run("do-another", {})
}
                "#;

        let result = parse_workflow(source).unwrap();

        assert_eq!(result.len(), 3);

        // First statement: fire-and-forget Task.run
        assert_eq!(result[0]["type"], "expression_statement");
        assert_eq!(result[0]["expression"]["type"], "function_call");
        assert_eq!(result[0]["expression"]["name"], json!(["Task", "run"]));
        assert_eq!(result[0]["expression"]["args"][0], "do-something");
        assert_eq!(result[0]["expression"]["args"][1]["hey"], "hello");

        // Second statement: await Task.run (await statement)
        assert_eq!(result[1]["type"], "await");
        assert_eq!(result[1]["expression"]["type"], "function_call");
        assert_eq!(result[1]["expression"]["name"], json!(["Task", "run"]));
        assert_eq!(result[1]["expression"]["args"][0], "await-task");
        assert_eq!(result[1]["expression"]["args"][1]["value"], 10);

        // Third statement: Task.run with empty object
        assert_eq!(result[2]["type"], "expression_statement");
        assert_eq!(result[2]["expression"]["type"], "function_call");
        assert_eq!(result[2]["expression"]["name"], json!(["Task", "run"]));
        assert_eq!(result[2]["expression"]["args"][0], "do-another");
        assert_eq!(result[2]["expression"]["args"][1], json!({}));
    }

    #[test]
    fn test_parse_await_task() {
        let source = r#"
workflow(ctx, inputs) {
  await Task.run("fetch-data", { "id": 123 })
  Task.run("log", { "msg": "fired and forgotten" })
}
                "#;

        let result = parse_workflow(source).unwrap();

        assert_eq!(result.len(), 2);

        // First task: await statement
        assert_eq!(result[0]["type"], "await");
        assert_eq!(result[0]["expression"]["type"], "function_call");
        assert_eq!(result[0]["expression"]["name"], json!(["Task", "run"]));
        assert_eq!(result[0]["expression"]["args"][0], "fetch-data");
        assert_eq!(result[0]["expression"]["args"][1]["id"], 123);

        // Second task: fire-and-forget (expression statement)
        assert_eq!(result[1]["type"], "expression_statement");
        assert_eq!(result[1]["expression"]["type"], "function_call");
        assert_eq!(result[1]["expression"]["name"], json!(["Task", "run"]));
        assert_eq!(result[1]["expression"]["args"][0], "log");
        assert_eq!(result[1]["expression"]["args"][1]["msg"], "fired and forgotten");
    }

    #[test]
    fn test_parse_task_without_inputs() {
        let source = r#"workflow(ctx, inputs) { Task.run("my-task", {}) }"#;
        let result = parse_workflow(source).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "expression_statement");
        assert_eq!(result[0]["expression"]["args"][1], json!({}));
    }

    #[test]
    fn test_parse_empty_lines() {
        let source = r#"
workflow(ctx, inputs) {
  Task.run("first", {})

  Task.run("second", {})

}
                "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_error_unknown_function() {
        let source = r#"unknown_func(123)"#;
        let result = parse_workflow(source);

        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        assert!(err_str.contains("Parse error") || err_str.contains("expected"));
    }

    #[test]
    fn test_parse_error_missing_closing_paren() {
        let source = r#"workflow(ctx, inputs) { Task.run("foo", {} }"#;
        let result = parse_workflow(source);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_error_invalid_json() {
        // Test invalid JSON syntax (missing closing brace)
        let source = r#"workflow(ctx, inputs) { Task.run("foo", { "key": "value" ) }"#;
        let result = parse_workflow(source);

        assert!(result.is_err());
        // Could be either InvalidJson or PestError depending on where it fails
    }

    #[test]
    fn test_parse_single_quotes() {
        let source = r#"workflow(ctx, inputs) { Task.run('my-task', {}) }"#;
        let result = parse_workflow(source).unwrap();

        assert_eq!(result[0]["expression"]["args"][0], "my-task");
    }

    #[test]
    fn test_parse_comments() {
        let source = r#"
workflow(ctx, inputs) {
  // This is a comment
  Task.run("first", {})
  // Another comment
  await Task.run("second", {})
}
                "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_nested_json() {
        let source = r#"workflow(ctx, inputs) { Task.run("process", { "user": { "name": "Alice", "age": 30 }, "count": 5 }) }"#;
        let result = parse_workflow(source).unwrap();

        assert_eq!(result[0]["expression"]["args"][1]["user"]["name"], "Alice");
        assert_eq!(result[0]["expression"]["args"][1]["user"]["age"], 30);
        assert_eq!(result[0]["expression"]["args"][1]["count"], 5);
    }

    #[test]
    fn test_parse_json_with_commas() {
        let source = r#"workflow(ctx, inputs) { Task.run("send", { "to": "user@example.com", "subject": "Hello", "body": "World" }) }"#;
        let result = parse_workflow(source).unwrap();

        assert_eq!(result[0]["expression"]["args"][1]["to"], "user@example.com");
        assert_eq!(result[0]["expression"]["args"][1]["subject"], "Hello");
        assert_eq!(result[0]["expression"]["args"][1]["body"], "World");
    }

    #[test]
    fn test_parse_variable_assignment() {
        let source = r#"
workflow(ctx, inputs) {
  let order_id = await Task.run("create_order", { "amount": 100 })
  let result = Task.run("log", { "msg": "test" })
}
                "#;

        let result = parse_workflow(source).unwrap();

        assert_eq!(result.len(), 2);

        // First assignment: with await
        assert_eq!(result[0]["type"], "assignment");
        assert_eq!(result[0]["left"]["type"], "variable");
        assert_eq!(result[0]["left"]["name"], "order_id");
        assert_eq!(result[0]["left"]["depth"], 0);
        assert_eq!(result[0]["right"]["type"], "await");
        assert_eq!(result[0]["right"]["expression"]["type"], "function_call");
        assert_eq!(result[0]["right"]["expression"]["name"], json!(["Task", "run"]));
        assert_eq!(result[0]["right"]["expression"]["args"][0], "create_order");
        assert_eq!(result[0]["right"]["expression"]["args"][1]["amount"], 100);

        // Second assignment: without await
        assert_eq!(result[1]["type"], "assignment");
        assert_eq!(result[1]["left"]["type"], "variable");
        assert_eq!(result[1]["left"]["name"], "result");
        assert_eq!(result[1]["left"]["depth"], 0);
        assert_eq!(result[1]["right"]["type"], "function_call");
        assert_eq!(result[1]["right"]["name"], json!(["Task", "run"]));
        assert_eq!(result[1]["right"]["args"][0], "log");
        assert_eq!(result[1]["right"]["args"][1]["msg"], "test");
    }

    #[test]
    fn test_parse_variable_names() {
        let source = r#"
workflow(ctx, inputs) {
  let my_var = Task.run("test", {})
  let _private = Task.run("test", {})
  let var123 = Task.run("test", {})
}
                "#;

        let result = parse_workflow(source).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0]["type"], "assignment");
        assert_eq!(result[0]["left"]["name"], "my_var");
        assert_eq!(result[1]["left"]["name"], "_private");
        assert_eq!(result[2]["left"]["name"], "var123");
    }

    #[test]
    fn test_parse_variable_references() {
        // Test bare identifier variable references in JSON
        let source = r#"
workflow(ctx, inputs) {
  let order_id = await Task.run("create_order", { "amount": 100 })
  await Task.run("charge", { "order_id": order_id, "amount": 50 })
}
                "#;

        let result = parse_workflow(source).unwrap();

        assert_eq!(result.len(), 2);

        // First task creates order_id
        assert_eq!(result[0]["type"], "assignment");
        assert_eq!(result[0]["left"]["name"], "order_id");
        assert_eq!(result[0]["right"]["expression"]["args"][1]["amount"], 100);

        // Second task uses order_id as a variable reference
        // Parser now annotates with scope depth for O(1) lookup
        assert_eq!(result[1]["type"], "await");
        let charge_args = &result[1]["expression"]["args"][1];
        assert_eq!(charge_args["order_id"]["type"], "variable");
        assert_eq!(charge_args["order_id"]["name"], "order_id");
        assert_eq!(charge_args["order_id"]["depth"], 0);
        assert_eq!(charge_args["amount"], 50);
    }

    #[test]
    fn test_parse_json_all_types() {
        let source = r#"
workflow(ctx, inputs) {
  let my_variable = Task.run("get_value", {})
  Task.run("test", {
    "string": "hello",
    "number": 42,
    "float": 3.14,
    "bool_true": true,
    "bool_false": false,
    "null_val": null,
    "var_ref": my_variable,
    "nested": { "key": "value" },
    "array": [1, 2, "three"]
  })
}
                "#;

        let result = parse_workflow(source).unwrap();

        assert_eq!(result.len(), 2);
        let inputs = &result[1]["expression"]["args"][1];

        assert_eq!(inputs["string"], "hello");
        assert_eq!(inputs["number"], 42);
        assert_eq!(inputs["float"], 3.14);
        assert_eq!(inputs["bool_true"], true);
        assert_eq!(inputs["bool_false"], false);
        assert_eq!(inputs["null_val"], JsonValue::Null);
        // Variable reference now annotated with scope depth and type
        assert_eq!(inputs["var_ref"]["type"], "variable");
        assert_eq!(inputs["var_ref"]["name"], "my_variable");
        assert_eq!(inputs["var_ref"]["depth"], 0);
        assert_eq!(inputs["nested"]["key"], "value");
        assert_eq!(inputs["array"][0], 1);
        assert_eq!(inputs["array"][1], 2);
        assert_eq!(inputs["array"][2], "three");
    }

    // === EXPLORATORY EDGE CASE TESTS ===
    // Testing unusual but potentially valid syntax to find parser bugs

    #[test]
    fn test_edge_trailing_comma_object() {
        // Trailing commas in objects - should this work?
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "a": 1, }) }"#;
        let result = parse_workflow(source);
        println!("Trailing comma in object: {:?}", result);
        // This might fail - trailing commas aren't standard JSON
    }

    #[test]
    fn test_edge_trailing_comma_array() {
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "arr": [1, 2,] }) }"#;
        let result = parse_workflow(source);
        println!("Trailing comma in array: {:?}", result);
    }

    #[test]
    fn test_edge_empty_array() {
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "arr": [] }) }"#;
        let result = parse_workflow(source).unwrap();
        let inputs = &result[0]["expression"]["args"][1];
        assert!(inputs["arr"].is_array());
        assert_eq!(inputs["arr"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_edge_array_only_variables() {
        let source = r#"
workflow(ctx, inputs) {
  let var1 = Task.run("t1", {})
  let var2 = Task.run("t2", {})
  let var3 = Task.run("t3", {})
  Task.run("t", { "arr": [var1, var2, var3] })
}
        "#;
        let result = parse_workflow(source).unwrap();
        let arr = &result[3]["expression"]["args"][1]["arr"];
        assert_eq!(arr[0]["type"], "variable");
        assert_eq!(arr[0]["name"], "var1");
        assert_eq!(arr[0]["depth"], 0);
        assert_eq!(arr[1]["type"], "variable");
        assert_eq!(arr[1]["name"], "var2");
        assert_eq!(arr[1]["depth"], 0);
        assert_eq!(arr[2]["type"], "variable");
        assert_eq!(arr[2]["name"], "var3");
        assert_eq!(arr[2]["depth"], 0);
    }

    #[test]
    fn test_edge_deeply_nested_json() {
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "a": { "b": { "c": { "d": { "e": { "f": "deep" } } } } } }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["a"]["b"]["c"]["d"]["e"]["f"], "deep");
    }

    #[test]
    fn test_edge_scientific_notation() {
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "n1": 1e10, "n2": 1e-10, "n3": 1.5E+5 }) }"#;
        let result = parse_workflow(source).unwrap();
        let inputs = &result[0]["expression"]["args"][1];
        // Check that scientific notation parses correctly
        assert!(inputs["n1"].is_number());
        assert!(inputs["n2"].is_number());
        assert!(inputs["n3"].is_number());
    }

    #[test]
    fn test_edge_negative_numbers() {
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "int": -42, "float": -3.14 }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["int"], -42);
        assert_eq!(result[0]["expression"]["args"][1]["float"], -3.14);
    }

    #[test]
    fn test_edge_no_spaces() {
        let source = r#"workflow(ctx, inputs) { let x=Task.run("t",{}) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["type"], "assignment");
        assert_eq!(result[0]["left"]["name"], "x");
    }

    #[test]
    fn test_edge_unicode_task_name() {
        let source = r#"workflow(ctx, inputs) { Task.run("😀", {}) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][0], "😀");
    }

    #[test]
    fn test_edge_special_chars_in_task_name() {
        let source = r#"workflow(ctx, inputs) { Task.run("my-task.name/v1:process", {}) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][0], "my-task.name/v1:process");
    }

    #[test]
    fn test_edge_escaped_chars_in_strings() {
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "msg": "He said \"hello\"", "path": "C:\\Users" }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["msg"], r#"He said "hello""#);
        assert_eq!(result[0]["expression"]["args"][1]["path"], r"C:\Users");
    }

    #[test]
    fn test_edge_variable_shadowing() {
        // Same variable name assigned twice - later should overwrite
        let source = r#"
workflow(ctx, inputs) {
  let x = Task.run("t1", {})
  let x = Task.run("t2", {})
  Task.run("t3", { "val": x })
}
                "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0]["type"], "assignment");
        assert_eq!(result[0]["left"]["name"], "x");
        assert_eq!(result[1]["type"], "assignment");
        assert_eq!(result[1]["left"]["name"], "x");
        // Variable x is found in symbol table at depth 0
        assert_eq!(result[2]["expression"]["args"][1]["val"]["type"], "variable");
        assert_eq!(result[2]["expression"]["args"][1]["val"]["name"], "x");
        assert_eq!(result[2]["expression"]["args"][1]["val"]["depth"], 0);
        // Note: executor should use the last assigned value
    }

    #[test]
    fn test_edge_empty_task_name() {
        let source = r#"workflow(ctx, inputs) { Task.run("", {}) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][0], "");
    }

    #[test]
    fn test_edge_empty_string_value() {
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "msg": "" }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["msg"], "");
    }

    #[test]
    fn test_edge_task_without_inputs() {
        // Task with only one argument (task name) should parse successfully
        // Semantic validator will check this is valid (Task.run accepts 1-2 args)
        let source = r#"workflow(ctx, inputs) { Task.run("task_name") }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["type"], "expression_statement");
        assert_eq!(result[0]["expression"]["type"], "function_call");
        assert_eq!(result[0]["expression"]["name"], json!(["Task", "run"]));
        assert_eq!(result[0]["expression"]["args"].as_array().unwrap().len(), 1);
        assert_eq!(result[0]["expression"]["args"][0], "task_name");
    }

    #[test]
    fn test_edge_reserved_word_variable_names() {
        // Reserved words from other languages should work as variable names
        let source = r#"
workflow(ctx, inputs) {
  let class = Task.run("t1", {})
  let import = Task.run("t2", {})
  let return = Task.run("t3", {})
}
                "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0]["type"], "assignment");
        assert_eq!(result[0]["left"]["name"], "class");
        assert_eq!(result[1]["type"], "assignment");
        assert_eq!(result[1]["left"]["name"], "import");
        assert_eq!(result[2]["type"], "assignment");
        assert_eq!(result[2]["left"]["name"], "return");
    }

    #[test]
    fn test_edge_nested_empty_structures() {
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "a": {}, "b": [] }) }"#;
        let result = parse_workflow(source).unwrap();
        assert!(result[0]["expression"]["args"][1]["a"].is_object());
        assert!(result[0]["expression"]["args"][1]["b"].is_array());
        assert_eq!(result[0]["expression"]["args"][1]["a"].as_object().unwrap().len(), 0);
        assert_eq!(result[0]["expression"]["args"][1]["b"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_edge_mixed_quote_types() {
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "key1": 'value1', 'key2': "value2" }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["key1"], "value1");
        assert_eq!(result[0]["expression"]["args"][1]["key2"], "value2");
    }

    #[test]
    fn test_edge_number_starting_with_dot() {
        // .5 should be valid (same as 0.5)
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "val": .5 }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["val"], 0.5);
    }

    #[test]
    fn test_edge_number_ending_with_dot() {
        // 5. should be valid (same as 5.0)
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "val": 5. }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["val"], 5.0);
    }

    #[test]
    fn test_edge_negative_number_with_dot() {
        // -.5 should work
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "val": -.5 }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["val"], -0.5);
    }

    #[test]
    fn test_edge_very_large_number() {
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "val": 999999999999999999999 }) }"#;
        let result = parse_workflow(source);
        // Large numbers might overflow or parse differently
        println!("Very large number: {:?}", result.as_ref().map(|v| &v[0]["inputs"]["val"]));
    }

    #[test]
    fn test_edge_zero_values() {
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "int": 0, "float": 0.0 }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["int"], 0);
        assert_eq!(result[0]["expression"]["args"][1]["float"], 0.0);
    }

    #[test]
    fn test_edge_whitespace_only_object() {
        let source = r#"workflow(ctx, inputs) { Task.run("t", {   }) }"#;
        let result = parse_workflow(source).unwrap();
        assert!(result[0]["expression"]["args"][1].is_object());
        assert_eq!(result[0]["expression"]["args"][1].as_object().unwrap().len(), 0);
    }

    #[test]
    fn test_edge_multiline_json() {
        let source = r#"
workflow(ctx, inputs) {
  Task.run("t", {
    "key1": "value1",
    "key2": "value2"
  })
}
                "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["key1"], "value1");
        assert_eq!(result[0]["expression"]["args"][1]["key2"], "value2");
    }

    #[test]
    fn test_edge_consecutive_commas() {
        // Double comma should fail
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "a": 1,, "b": 2 }) }"#;
        let result = parse_workflow(source);
        assert!(result.is_err(), "Consecutive commas should be invalid");
    }

    #[test]
    fn test_edge_unquoted_variable_value() {
        // Variable as value (unquoted identifier) should work
        let source = r#"
workflow(ctx, inputs) {
  let my_var = Task.run("get", {})
  Task.run("t", { "val": my_var })
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[1]["expression"]["args"][1]["val"]["type"], "variable");
        assert_eq!(result[1]["expression"]["args"][1]["val"]["name"], "my_var");
        assert_eq!(result[1]["expression"]["args"][1]["val"]["depth"], 0);
    }

    #[test]
    fn test_edge_tabs_as_whitespace() {
        let source = "workflow(ctx, inputs) { let\tx\t=\tTask.run(\"t\",\t{}) }";
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["type"], "assignment");
        assert_eq!(result[0]["left"]["name"], "x");
    }

    #[test]
    fn test_edge_trailing_comment() {
        let source = r#"
workflow(ctx, inputs) {
  Task.run("t", {}) // trailing comment
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_edge_string_with_escapes() {
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "msg": "\n\t\r" }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["msg"], "\n\t\r");
    }

    #[test]
    fn test_edge_empty_workflow() {
        let source = "workflow(ctx, inputs) {}";
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_edge_only_comments() {
        let source = r#"
workflow(ctx, inputs) {
  // Just comments
  // No actual code
}
                "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_edge_hash_comments_disallowed() {
        // Hash comments should be explicitly disallowed
        let source = r#"workflow(ctx, inputs) { Task.run("t1", {}) # this should fail }"#;
        let result = parse_workflow(source);
        assert!(result.is_err(), "Hash comments should not be allowed");

        // Check error message
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Use '//' for comments"), "Error message should mention using //");
    }

    #[test]
    fn test_edge_windows_line_endings() {
        let source = "workflow(ctx, inputs) {\r\n  task(\"t1\", {})\r\n  task(\"t2\", {})\r\n}";
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_edge_multiple_statements_one_line() {
        // This should fail - statements must be separated by newlines
        let source = r#"workflow(ctx, inputs) { let x = Task.run("t1", {}) let y = Task.run("t2", {}) }"#;
        let result = parse_workflow(source);
        assert!(result.is_err(), "Multiple statements on one line should fail");
    }

    #[test]
    fn test_edge_semicolons_disallowed() {
        // Semicolons should be explicitly disallowed (unlike JS)
        let source = r#"workflow(ctx, inputs) { Task.run("t1", {}); }"#;
        let result = parse_workflow(source);
        assert!(result.is_err(), "Semicolons should not be allowed");

        // Check error message
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Semicolons are not allowed"), "Error message should mention semicolons");
    }

    #[test]
    fn test_edge_multiple_statements_with_semicolons() {
        // Multiple statements separated by semicolons should fail
        let source = r#"workflow(ctx, inputs) { Task.run("t1", {}); Task.run("t2", {}) }"#;
        let result = parse_workflow(source);
        assert!(result.is_err(), "Semicolon-separated statements should fail");
    }

    #[test]
    fn test_edge_hex_numbers() {
        // Hex numbers - now supported!
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "val": 0x1234, "color": 0xFF00FF }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["val"], 0x1234);  // 4660
        assert_eq!(result[0]["expression"]["args"][1]["color"], 0xFF00FF);  // 16711935
    }

    #[test]
    fn test_edge_binary_numbers() {
        // Binary numbers - now supported!
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "flags": 0b1010, "mask": 0b11110000 }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["flags"], 0b1010);  // 10
        assert_eq!(result[0]["expression"]["args"][1]["mask"], 0b11110000);  // 240
    }

    #[test]
    fn test_edge_numbers_with_underscores() {
        // Numbers with underscores - now supported for readability!
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "big": 1_000_000, "pi": 3.14_15_92 }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["big"], 1_000_000);
        assert_eq!(result[0]["expression"]["args"][1]["pi"], 3.141592);
    }

    #[test]
    fn test_edge_hex_with_underscores() {
        // Combine hex and underscores
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "addr": 0xDEAD_BEEF }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["addr"], 3735928559i64);
    }

    #[test]
    fn test_edge_negative_hex() {
        // Negative hex numbers
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "val": -0xFF }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["val"], -255);
    }

    #[test]
    fn test_edge_unquoted_keys() {
        // Unquoted keys (JSON5 style)
        let source = r#"workflow(ctx, inputs) { Task.run("t", { name: "Alice", age: 30, active: true }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["name"], "Alice");
        assert_eq!(result[0]["expression"]["args"][1]["age"], 30);
        assert_eq!(result[0]["expression"]["args"][1]["active"], true);
    }

    #[test]
    fn test_edge_mixed_quoted_unquoted_keys() {
        // Mix quoted and unquoted keys
        let source = r#"workflow(ctx, inputs) { Task.run("t", { "user-id": 123, name: "Alice", "is-admin": false }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["user-id"], 123);
        assert_eq!(result[0]["expression"]["args"][1]["name"], "Alice");
        assert_eq!(result[0]["expression"]["args"][1]["is-admin"], false);
    }

    #[test]
    fn test_edge_unquoted_keys_with_variables() {
        // Unquoted keys with variable values
        let source = r#"
workflow(ctx, inputs) {
  let user_id = Task.run("get_user", {})
  let my_config = Task.run("get_config", {})
  Task.run("t", { userId: user_id, config: my_config })
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[2]["expression"]["args"][1]["userId"]["type"], "variable");
        assert_eq!(result[2]["expression"]["args"][1]["userId"]["name"], "user_id");
        assert_eq!(result[2]["expression"]["args"][1]["userId"]["depth"], 0);
        assert_eq!(result[2]["expression"]["args"][1]["config"]["type"], "variable");
        assert_eq!(result[2]["expression"]["args"][1]["config"]["name"], "my_config");
        assert_eq!(result[2]["expression"]["args"][1]["config"]["depth"], 0);
    }

    #[test]
    fn test_edge_nested_unquoted_keys() {
        // Nested objects with unquoted keys
        let source = r#"workflow(ctx, inputs) { Task.run("t", { user: { name: "Alice", age: 30 }, count: 5 }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["expression"]["args"][1]["user"]["name"], "Alice");
        assert_eq!(result[0]["expression"]["args"][1]["user"]["age"], 30);
        assert_eq!(result[0]["expression"]["args"][1]["count"], 5);
    }

    #[test]
    fn test_showcase_workflow() {
        // Integration test for complete feature showcase
        let source = std::fs::read_to_string("../python/examples/workflows/showcase.flow").unwrap();
        let result = parse_workflow(&source);
        assert!(result.is_ok(), "Showcase workflow failed to parse: {:?}", result.err());
        let steps = result.unwrap();
        assert!(steps.len() > 10, "Expected at least 10 steps in showcase");
    }

    #[test]
    fn test_inputs_member_access() {
        // Test inputs.fieldName syntax
        let source = r#"
workflow(ctx, inputs) {
  await Task.run("process", { userId: inputs.userId, orderId: inputs.orderId })
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        // Member access should be structured JSON
        assert_eq!(result[0]["expression"]["args"][1]["userId"]["base"], "inputs");
        assert_eq!(result[0]["expression"]["args"][1]["userId"]["path"][0]["type"], "dot");
        assert_eq!(result[0]["expression"]["args"][1]["userId"]["path"][0]["value"], "userId");

        assert_eq!(result[0]["expression"]["args"][1]["orderId"]["base"], "inputs");
        assert_eq!(result[0]["expression"]["args"][1]["orderId"]["path"][0]["type"], "dot");
        assert_eq!(result[0]["expression"]["args"][1]["orderId"]["path"][0]["value"], "orderId");
    }

    #[test]
    fn test_ctx_member_access() {
        // Test ctx.workflowId syntax
        let source = r#"
workflow(ctx, inputs) {
  await Task.run("log", { workflowId: ctx.workflowId })
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        // ctx.workflowId should be stored in structured format
        let args = &result[0]["expression"]["args"][1];
        assert_eq!(args["workflowId"]["base"], "ctx");
        assert_eq!(args["workflowId"]["path"][0]["type"], "dot");
        assert_eq!(args["workflowId"]["path"][0]["value"], "workflowId");
    }

    #[test]
    fn test_mixed_member_access_and_variables() {
        // Test mixing inputs.*, ctx.*, and bare variable references
        let source = r#"
workflow(ctx, inputs) {
  let result = await Task.run("validate", { userId: inputs.userId })
  await Task.run("process", {
    validationResult: result,
    workflow: ctx.workflowId,
    order: inputs.orderId
  })
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 2);

        // First statement: assignment with await
        assert_eq!(result[0]["type"], "assignment");
        let first_args = &result[0]["right"]["expression"]["args"][1];
        assert_eq!(first_args["userId"]["base"], "inputs");
        assert_eq!(first_args["userId"]["path"][0]["type"], "dot");
        assert_eq!(first_args["userId"]["path"][0]["value"], "userId");

        // Second statement: mixed references
        assert_eq!(result[1]["type"], "await");
        let second_args = &result[1]["expression"]["args"][1];
        // Bare variable reference now annotated with scope depth and type
        assert_eq!(second_args["validationResult"]["type"], "variable");
        assert_eq!(second_args["validationResult"]["name"], "result");
        assert_eq!(second_args["validationResult"]["depth"], 0);
        assert_eq!(second_args["workflow"]["base"], "ctx");
        assert_eq!(second_args["workflow"]["path"][0]["type"], "dot");
        assert_eq!(second_args["workflow"]["path"][0]["value"], "workflowId");
        assert_eq!(second_args["order"]["base"], "inputs");
        assert_eq!(second_args["order"]["path"][0]["type"], "dot");
        assert_eq!(second_args["order"]["path"][0]["value"], "orderId");
    }

    #[test]
    fn test_dollar_sign_escape() {
        // Test that $$ in literal strings becomes a single $
        let source = r#"workflow(ctx, inputs) {
  Task.run("test", { "price": "$$99.99", "note": "Only $$50!" })
}"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        // $$ should be unescaped to single $
        let args = &result[0]["expression"]["args"][1];
        assert_eq!(args["price"], "$99.99");
        assert_eq!(args["note"], "Only $50!");
    }

    // === IF/ELSE CONDITIONAL TESTS ===

    #[test]
    fn test_simple_if_statement() {
        let source = r#"
workflow(ctx, inputs) {
  if (inputs.status == "success") {
    await Task.run("send_success", {})
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        assert_eq!(result[0]["type"], "if");
        assert_eq!(result[0]["condition"]["type"], "comparison");
        assert_eq!(result[0]["condition"]["operator"], "==");
        // Member access is now in structured format
        assert_eq!(result[0]["condition"]["left"]["base"], "inputs");
        assert_eq!(result[0]["condition"]["left"]["path"][0]["type"], "dot");
        assert_eq!(result[0]["condition"]["left"]["path"][0]["value"], "status");
        assert_eq!(result[0]["condition"]["right"], "success");

        let then_stmts = result[0]["then_statements"].as_array().unwrap();
        assert_eq!(then_stmts.len(), 1);
        assert_eq!(then_stmts[0]["type"], "await");
        assert_eq!(then_stmts[0]["expression"]["type"], "function_call");
        assert_eq!(then_stmts[0]["expression"]["args"][0], "send_success");
    }

    #[test]
    fn test_if_else_statement() {
        let source = r#"
workflow(ctx, inputs) {
  if (inputs.amount > 100) {
    await Task.run("premium_processing", {})
  } else {
    await Task.run("standard_processing", {})
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        assert_eq!(result[0]["type"], "if");
        assert_eq!(result[0]["condition"]["type"], "comparison");
        assert_eq!(result[0]["condition"]["operator"], ">");
        assert_eq!(result[0]["condition"]["left"]["base"], "inputs");
        assert_eq!(result[0]["condition"]["left"]["path"][0]["type"], "dot");
        assert_eq!(result[0]["condition"]["left"]["path"][0]["value"], "amount");
        assert_eq!(result[0]["condition"]["right"], 100);

        let then_stmts = result[0]["then_statements"].as_array().unwrap();
        assert_eq!(then_stmts.len(), 1);
        assert_eq!(then_stmts[0]["type"], "await");
        assert_eq!(then_stmts[0]["expression"]["args"][0], "premium_processing");

        let else_stmts = result[0]["else_statements"].as_array().unwrap();
        assert_eq!(else_stmts.len(), 1);
        assert_eq!(else_stmts[0]["type"], "await");
        assert_eq!(else_stmts[0]["expression"]["args"][0], "standard_processing");
    }

    #[test]
    fn test_if_with_member_access() {
        let source = r#"
workflow(ctx, inputs) {
  if (payment.status == "completed") {
    await Task.run("ship_order", {})
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        assert_eq!(result[0]["condition"]["left"]["base"], "payment");
        assert_eq!(result[0]["condition"]["left"]["path"][0]["type"], "dot");
        assert_eq!(result[0]["condition"]["left"]["path"][0]["value"], "status");
        assert_eq!(result[0]["condition"]["right"], "completed");
    }

    #[test]
    fn test_if_with_inputs_access() {
        let source = r#"
workflow(ctx, inputs) {
  if (inputs.userId == 123) {
    await Task.run("admin_task", {})
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        assert_eq!(result[0]["condition"]["left"]["base"], "inputs");
        assert_eq!(result[0]["condition"]["left"]["path"][0]["type"], "dot");
        assert_eq!(result[0]["condition"]["left"]["path"][0]["value"], "userId");
        assert_eq!(result[0]["condition"]["right"], 123);
    }

    #[test]
    fn test_if_with_and_operator() {
        let source = r#"
workflow(ctx, inputs) {
  if (inputs.amount > 100 && inputs.status == "approved") {
    await Task.run("process", {})
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        assert_eq!(result[0]["condition"]["type"], "and");
        let operands = result[0]["condition"]["operands"].as_array().unwrap();
        assert_eq!(operands.len(), 2);

        assert_eq!(operands[0]["operator"], ">");
        assert_eq!(operands[0]["left"]["base"], "inputs");
        assert_eq!(operands[0]["left"]["path"][0]["type"], "dot");
        assert_eq!(operands[0]["left"]["path"][0]["value"], "amount");
        assert_eq!(operands[0]["right"], 100);

        assert_eq!(operands[1]["operator"], "==");
        assert_eq!(operands[1]["left"]["base"], "inputs");
        assert_eq!(operands[1]["left"]["path"][0]["type"], "dot");
        assert_eq!(operands[1]["left"]["path"][0]["value"], "status");
        assert_eq!(operands[1]["right"], "approved");
    }

    #[test]
    fn test_if_with_or_operator() {
        let source = r#"
workflow(ctx, inputs) {
  if (inputs.status == "failed" || inputs.status == "cancelled") {
    await Task.run("cleanup", {})
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        assert_eq!(result[0]["condition"]["type"], "or");
        let operands = result[0]["condition"]["operands"].as_array().unwrap();
        assert_eq!(operands.len(), 2);

        assert_eq!(operands[0]["operator"], "==");
        assert_eq!(operands[0]["left"]["base"], "inputs");
        assert_eq!(operands[0]["left"]["path"][0]["type"], "dot");
        assert_eq!(operands[0]["left"]["path"][0]["value"], "status");
        assert_eq!(operands[0]["right"], "failed");

        assert_eq!(operands[1]["operator"], "==");
        assert_eq!(operands[1]["left"]["base"], "inputs");
        assert_eq!(operands[1]["left"]["path"][0]["type"], "dot");
        assert_eq!(operands[1]["left"]["path"][0]["value"], "status");
        assert_eq!(operands[1]["right"], "cancelled");
    }

    #[test]
    fn test_if_with_complex_condition() {
        let source = r#"
workflow(ctx, inputs) {
  if ((inputs.amount > 100 && inputs.priority == "high") || inputs.urgent == true) {
    await Task.run("fast_track", {})
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        assert_eq!(result[0]["condition"]["type"], "or");
    }

    #[test]
    fn test_if_with_multiple_statements() {
        let source = r#"
workflow(ctx, inputs) {
  if (inputs.status == "success") {
    await Task.run("send_notification", {})
    await Task.run("update_stats", {})
    Task.run("log_event", {})
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        let then_stmts = result[0]["then_statements"].as_array().unwrap();
        assert_eq!(then_stmts.len(), 3);
        assert_eq!(then_stmts[0]["type"], "await");
        assert_eq!(then_stmts[0]["expression"]["args"][0], "send_notification");
        assert_eq!(then_stmts[1]["type"], "await");
        assert_eq!(then_stmts[1]["expression"]["args"][0], "update_stats");
        assert_eq!(then_stmts[2]["type"], "expression_statement");
        assert_eq!(then_stmts[2]["expression"]["args"][0], "log_event");
    }

    #[test]
    fn test_nested_if_statements() {
        let source = r#"
workflow(ctx, inputs) {
  if (inputs.level == 1) {
    if (inputs.sublevel == "a") {
      await Task.run("nested_task", {})
    }
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        assert_eq!(result[0]["type"], "if");
        let then_stmts = result[0]["then_statements"].as_array().unwrap();
        assert_eq!(then_stmts.len(), 1);
        assert_eq!(then_stmts[0]["type"], "if");
    }

    #[test]
    fn test_if_with_all_comparison_operators() {
        let source = r#"
workflow(ctx, inputs) {
  if (inputs.a == 1) { Task.run("t1", {}) }
  if (inputs.b != 2) { Task.run("t2", {}) }
  if (inputs.c < 3) { Task.run("t3", {}) }
  if (inputs.d > 4) { Task.run("t4", {}) }
  if (inputs.e <= 5) { Task.run("t5", {}) }
  if (inputs.f >= 6) { Task.run("t6", {}) }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 6);

        assert_eq!(result[0]["condition"]["operator"], "==");
        assert_eq!(result[1]["condition"]["operator"], "!=");
        assert_eq!(result[2]["condition"]["operator"], "<");
        assert_eq!(result[3]["condition"]["operator"], ">");
        assert_eq!(result[4]["condition"]["operator"], "<=");
        assert_eq!(result[5]["condition"]["operator"], ">=");
    }

    #[test]
    fn test_if_with_variable_assignment() {
        let source = r#"
workflow(ctx, inputs) {
  let result = await Task.run("check_status", {})
  if (result.success == true) {
    await Task.run("continue", {})
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 2);

        assert_eq!(result[0]["type"], "assignment");
        assert_eq!(result[0]["left"]["type"], "variable");
        assert_eq!(result[0]["left"]["name"], "result");

        assert_eq!(result[1]["type"], "if");
        assert_eq!(result[1]["condition"]["left"]["base"], "result");
        assert_eq!(result[1]["condition"]["left"]["path"][0]["type"], "dot");
        assert_eq!(result[1]["condition"]["left"]["path"][0]["value"], "success");
        assert_eq!(result[1]["condition"]["right"], true);
    }

    #[test]
    fn test_if_with_boolean_values() {
        let source = r#"
workflow(ctx, inputs) {
  if (inputs.enabled == true) {
    await Task.run("process", {})
  } else {
    await Task.run("skip", {})
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        assert_eq!(result[0]["condition"]["left"]["base"], "inputs");
        assert_eq!(result[0]["condition"]["left"]["path"][0]["type"], "dot");
        assert_eq!(result[0]["condition"]["left"]["path"][0]["value"], "enabled");
        assert_eq!(result[0]["condition"]["right"], true);
    }

    #[test]
    fn test_if_with_null_comparison() {
        let source = r#"
workflow(ctx, inputs) {
  if (inputs.optionalValue == null) {
    await Task.run("handle_missing", {})
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        assert_eq!(result[0]["condition"]["left"]["base"], "inputs");
        assert_eq!(result[0]["condition"]["left"]["path"][0]["type"], "dot");
        assert_eq!(result[0]["condition"]["left"]["path"][0]["value"], "optionalValue");
        assert_eq!(result[0]["condition"]["right"], JsonValue::Null);
    }

    #[test]
    fn test_payment_conditional_workflow() {
        // Test that the payment_conditional.flow example parses correctly
        let source = r#"
workflow(ctx, inputs) {
  if (inputs.amount > 1000) {
    await Task.run("verify_identity", { userId: inputs.userId })
    let paymentResult = await Task.run("process_premium_payment", {
      userId: inputs.userId,
      amount: inputs.amount
    })
  } else {
    let paymentResult = await Task.run("process_standard_payment", {
      userId: inputs.userId,
      amount: inputs.amount
    })
  }
  if (paymentResult.status == "success") {
    await Task.run("send_receipt", {
      userId: inputs.userId,
      transactionId: paymentResult.transactionId
    })
  }
}
        "#;
        let result = parse_workflow(source);
        assert!(result.is_ok(), "Payment conditional workflow should parse: {:?}", result.err());
        let steps = result.unwrap();
        assert!(steps.len() >= 2, "Should have at least 2 if statements");
    }

    #[test]
    fn test_user_onboarding_workflow() {
        // Test complex conditions with && and || operators
        let source = r#"
workflow(ctx, inputs) {
  let user = await Task.run("create_user_account", {
    email: inputs.email,
    name: inputs.name
  })

  if (inputs.referralCode != null && inputs.signupSource == "partner") {
    await Task.run("activate_premium_trial", { userId: user.id })
  } else {
    await Task.run("send_standard_welcome", { email: inputs.email })
  }

  if (inputs.country == "US" || inputs.country == "CA") {
    await Task.run("setup_north_america_features", { userId: user.id })
  }
}
        "#;
        let result = parse_workflow(source);
        assert!(result.is_ok(), "User onboarding workflow should parse: {:?}", result.err());
    }

    // === FOR LOOP TESTS ===

    #[test]
    fn test_simple_for_loop_inline_array() {
        let source = r#"
workflow(ctx, inputs) {
  for (let item in [1, 2, 3]) {
    Task.run("process", { value: item })
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        assert_eq!(result[0]["type"], "for");
        assert_eq!(result[0]["loop_variable"], "item");

        // Check iterable is inline array
        assert_eq!(result[0]["iterable"]["type"], "array");
        let arr = result[0]["iterable"]["value"].as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0], 1);
        assert_eq!(arr[1], 2);
        assert_eq!(arr[2], 3);

        // Check body
        let body = result[0]["body_statements"].as_array().unwrap();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0]["type"], "expression_statement");
        assert_eq!(body[0]["expression"]["type"], "function_call");
        assert_eq!(body[0]["expression"]["name"], json!(["Task", "run"]));
        assert_eq!(body[0]["expression"]["args"][0], "process");

        // Check that loop variable is used in body and annotated
        assert_eq!(body[0]["expression"]["args"][1]["value"]["type"], "variable");
        assert_eq!(body[0]["expression"]["args"][1]["value"]["name"], "item");
        assert_eq!(body[0]["expression"]["args"][1]["value"]["depth"], 1); // Loop creates depth 1 scope
    }

    #[test]
    fn test_for_loop_with_member_access_iterable() {
        let source = r#"
workflow(ctx, inputs) {
  for (let item in inputs.items) {
    Task.run("process", { value: item })
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        assert_eq!(result[0]["type"], "for");
        assert_eq!(result[0]["iterable"]["type"], "member_access");
        assert_eq!(result[0]["iterable"]["value"]["base"], "inputs");
        assert_eq!(result[0]["iterable"]["value"]["path"][0]["type"], "dot");
        assert_eq!(result[0]["iterable"]["value"]["path"][0]["value"], "items");
    }

    #[test]
    fn test_for_loop_with_variable_iterable() {
        let source = r#"
workflow(ctx, inputs) {
  let items = Task.run("get_items", {})
  for (let item in items) {
    Task.run("process", { value: item })
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 2);

        assert_eq!(result[1]["type"], "for");
        assert_eq!(result[1]["iterable"]["type"], "variable");
        assert_eq!(result[1]["iterable"]["value"]["var"], "items");
        assert_eq!(result[1]["iterable"]["value"]["depth"], 0);
    }

    #[test]
    fn test_nested_for_loops() {
        let source = r#"
workflow(ctx, inputs) {
  for (let outer in [1, 2]) {
    for (let inner in [3, 4]) {
      Task.run("process", { o: outer, i: inner })
    }
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        // Check outer loop
        assert_eq!(result[0]["type"], "for");
        assert_eq!(result[0]["loop_variable"], "outer");

        // Check inner loop is in outer's body
        let outer_body = result[0]["body_statements"].as_array().unwrap();
        assert_eq!(outer_body.len(), 1);
        assert_eq!(outer_body[0]["type"], "for");
        assert_eq!(outer_body[0]["loop_variable"], "inner");

        // Check variables in inner loop body are annotated with correct depths
        let inner_body = outer_body[0]["body_statements"].as_array().unwrap();
        assert_eq!(inner_body.len(), 1);
        assert_eq!(inner_body[0]["type"], "expression_statement");
        assert_eq!(inner_body[0]["expression"]["args"][1]["o"]["type"], "variable");
        assert_eq!(inner_body[0]["expression"]["args"][1]["o"]["name"], "outer");
        assert_eq!(inner_body[0]["expression"]["args"][1]["o"]["depth"], 1); // Outer loop depth
        assert_eq!(inner_body[0]["expression"]["args"][1]["i"]["type"], "variable");
        assert_eq!(inner_body[0]["expression"]["args"][1]["i"]["name"], "inner");
        assert_eq!(inner_body[0]["expression"]["args"][1]["i"]["depth"], 2); // Inner loop depth
    }

    #[test]
    fn test_for_loop_with_await() {
        let source = r#"
workflow(ctx, inputs) {
  for (let order in inputs.orders) {
    await Task.run("processOrder", { orderId: order.id })
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "for");
        assert_eq!(result[0]["loop_variable"], "order");

        let body = result[0]["body_statements"].as_array().unwrap();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0]["type"], "await");
        assert_eq!(body[0]["expression"]["type"], "function_call");
        assert_eq!(body[0]["expression"]["name"], json!(["Task", "run"]));
        assert_eq!(body[0]["expression"]["args"][0], "processOrder");
    }

    #[test]
    fn test_for_loop_with_mixed_await() {
        let source = r#"
workflow(ctx, inputs) {
  for (let item in inputs.items) {
    Task.run("log", { value: item })
    await Task.run("process", { value: item })
    Task.run("notify", { value: item })
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "for");

        let body = result[0]["body_statements"].as_array().unwrap();
        assert_eq!(body.len(), 3);

        // First task - fire-and-forget
        assert_eq!(body[0]["type"], "expression_statement");
        assert_eq!(body[0]["expression"]["type"], "function_call");
        assert_eq!(body[0]["expression"]["args"][0], "log");

        // Second task - await
        assert_eq!(body[1]["type"], "await");
        assert_eq!(body[1]["expression"]["type"], "function_call");
        assert_eq!(body[1]["expression"]["args"][0], "process");

        // Third task - fire-and-forget
        assert_eq!(body[2]["type"], "expression_statement");
        assert_eq!(body[2]["expression"]["type"], "function_call");
        assert_eq!(body[2]["expression"]["args"][0], "notify");
    }

    #[test]
    fn test_for_loop_examples_workflow() {
        let source = std::fs::read_to_string("../python/examples/workflows/for_loop_examples.flow")
            .expect("Failed to read for_loop_examples.flow");
        let result = parse_workflow(&source);
        assert!(result.is_ok(), "For loop examples workflow should parse: {:?}", result.err());

        let statements = result.unwrap();

        // Should have 6 main examples plus final completion task
        // Each example is either a for loop or let statement
        assert!(statements.len() >= 7, "Expected at least 7 top-level statements");

        // Check that we have for loops
        let for_loops = statements.iter().filter(|s| s["type"] == "for").count();
        assert!(for_loops >= 5, "Expected at least 5 for loops");
    }

    // === BREAK/CONTINUE TESTS ===

    #[test]
    fn test_for_loop_with_break() {
        let source = r#"
workflow(ctx, inputs) {
  for (let item in [1, 2, 3, 4, 5]) {
    if (item == 3) {
      break
    }
    Task.run("process", { value: item })
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "for");

        let body = result[0]["body_statements"].as_array().unwrap();
        assert_eq!(body.len(), 2);

        // First statement is if
        assert_eq!(body[0]["type"], "if");
        let then_stmts = body[0]["then_statements"].as_array().unwrap();
        assert_eq!(then_stmts.len(), 1);
        assert_eq!(then_stmts[0]["type"], "break");

        // Second statement is expression statement (Task.run call)
        assert_eq!(body[1]["type"], "expression_statement");
        assert_eq!(body[1]["expression"]["type"], "function_call");
    }

    #[test]
    fn test_for_loop_with_continue() {
        let source = r#"
workflow(ctx, inputs) {
  for (let item in inputs.items) {
    if (item.skip == true) {
      continue
    }
    await Task.run("process", { value: item })
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "for");

        let body = result[0]["body_statements"].as_array().unwrap();
        assert_eq!(body.len(), 2);

        // First statement is if with continue
        assert_eq!(body[0]["type"], "if");
        let then_stmts = body[0]["then_statements"].as_array().unwrap();
        assert_eq!(then_stmts.len(), 1);
        assert_eq!(then_stmts[0]["type"], "continue");

        // Second statement is await statement
        assert_eq!(body[1]["type"], "await");
        assert_eq!(body[1]["expression"]["type"], "function_call");
    }

    #[test]
    fn test_for_loop_with_break_and_continue() {
        let source = r#"
workflow(ctx, inputs) {
  for (let item in [1, 2, 3, 4, 5]) {
    if (item > 10) {
      break
    }
    if (item == 2) {
      continue
    }
    Task.run("process", { value: item })
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        let body = result[0]["body_statements"].as_array().unwrap();
        assert_eq!(body.len(), 3);

        // First if has break
        assert_eq!(body[0]["type"], "if");
        assert_eq!(body[0]["then_statements"][0]["type"], "break");

        // Second if has continue
        assert_eq!(body[1]["type"], "if");
        assert_eq!(body[1]["then_statements"][0]["type"], "continue");

        // Third is expression statement (Task.run call)
        assert_eq!(body[2]["type"], "expression_statement");
        assert_eq!(body[2]["expression"]["type"], "function_call");
    }

    #[test]
    fn test_nested_loop_with_break() {
        let source = r#"
workflow(ctx, inputs) {
  for (let outer in [1, 2, 3]) {
    for (let inner in [4, 5, 6]) {
      if (inner == 5) {
        break
      }
      Task.run("process", { o: outer, i: inner })
    }
  }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        // Check outer loop
        let outer_body = result[0]["body_statements"].as_array().unwrap();
        assert_eq!(outer_body.len(), 1);
        assert_eq!(outer_body[0]["type"], "for");

        // Check inner loop has break
        let inner_body = outer_body[0]["body_statements"].as_array().unwrap();
        assert_eq!(inner_body.len(), 2);
        assert_eq!(inner_body[0]["type"], "if");
        assert_eq!(inner_body[0]["then_statements"][0]["type"], "break");
    }

    #[test]
    fn test_break_in_if_inside_loop_succeeds() {
        // This should work - break inside if, inside loop
        let source = r#"
workflow(ctx, inputs) {
  for (let item in [1, 2, 3]) {
    if (item == 2) {
      break
    }
  }
}
        "#;
        let result = parse_workflow(source);
        assert!(result.is_ok(), "Break inside if inside loop should work: {:?}", result.err());
    }

    // === RETURN STATEMENT TESTS ===

    #[test]
    fn test_return_without_value() {
        let source = r#"
workflow(ctx, inputs) {
  Task.run("start", {})
  return
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[1]["type"], "return");
        assert_eq!(result[1]["right"], JsonValue::Null);
    }

    #[test]
    fn test_return_with_string() {
        let source = r#"
workflow(ctx, inputs) {
  return "success"
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "return");
        assert_eq!(result[0]["value"], "success");
    }

    #[test]
    fn test_return_with_object() {
        let source = r#"
workflow(ctx, inputs) {
  return { status: "success", code: 200 }
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "return");
        assert_eq!(result[0]["value"]["status"], "success");
        assert_eq!(result[0]["value"]["code"], 200);
    }

    #[test]
    fn test_return_with_variable() {
        let source = r#"
workflow(ctx, inputs) {
  let result = await Task.run("compute", {})
  return result
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[1]["type"], "return");
        assert_eq!(result[1]["value"]["type"], "variable");
        assert_eq!(result[1]["value"]["name"], "result");
        assert_eq!(result[1]["value"]["depth"], 0);
    }

    #[test]
    fn test_return_in_if_statement() {
        let source = r#"
workflow(ctx, inputs) {
  if (inputs.shouldReturn == true) {
    return "early exit"
  }
  Task.run("continue", {})
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["type"], "if");

        let then_stmts = result[0]["then_statements"].as_array().unwrap();
        assert_eq!(then_stmts.len(), 1);
        assert_eq!(then_stmts[0]["type"], "return");
        assert_eq!(then_stmts[0]["value"], "early exit");
    }

    #[test]
    fn test_return_in_loop() {
        let source = r#"
workflow(ctx, inputs) {
  for (let item in inputs.items) {
    if (item.isTarget == true) {
      return item
    }
  }
  return null
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 2);

        // Check for loop
        assert_eq!(result[0]["type"], "for");
        let body = result[0]["body_statements"].as_array().unwrap();
        assert_eq!(body[0]["type"], "if");

        let then_stmts = body[0]["then_statements"].as_array().unwrap();
        assert_eq!(then_stmts[0]["type"], "return");
        assert_eq!(then_stmts[0]["value"]["type"], "variable");
        assert_eq!(then_stmts[0]["value"]["name"], "item");

        // Check final return
        assert_eq!(result[1]["type"], "return");
        assert_eq!(result[1]["value"], JsonValue::Null);
    }

    #[test]
    fn test_workflow_without_return_is_valid() {
        // Workflow without explicit return should be valid
        let source = r#"
workflow(ctx, inputs) {
  await Task.run("step1", {})
  await Task.run("step2", {})
  Task.run("step3", {})
}
        "#;
        let result = parse_workflow(source);
        assert!(result.is_ok(), "Workflow without return should be valid");

        let statements = result.unwrap();
        assert_eq!(statements.len(), 3);

        // None of the statements should be return
        for stmt in statements {
            assert_ne!(stmt["type"], "return");
        }
    }

    #[test]
    fn test_numeric_member_access_rejected() {
        // data.0 should be rejected - identifiers can't start with numbers
        let workflow = r#"
            task foo:
                let data = {"0": "value"}
                let x = data.0
        "#;

        let result = parse_workflow(workflow);
        assert!(result.is_err(), "data.0 should be rejected by parser");
    }

    #[test]
    fn test_bracket_notation_array_index() {
        let workflow = r#"
workflow(ctx, inputs) {
  let item = inputs.items[0]
  let name = inputs.items[1].name
}
        "#;
        let result = parse_workflow(workflow).unwrap();
        assert_eq!(result.len(), 2);

        // First assignment: inputs.items[0]
        assert_eq!(result[0]["type"], "assignment");
        assert_eq!(result[0]["left"]["name"], "item");
        let first_value = &result[0]["right"];
        assert_eq!(first_value["base"], "inputs");
        let path = first_value["path"].as_array().unwrap();
        assert_eq!(path[0]["type"], "dot");
        assert_eq!(path[0]["value"], "items");
        assert_eq!(path[1]["type"], "index");
        assert_eq!(path[1]["value"], 0);

        // Second assignment: inputs.items[1].name
        assert_eq!(result[1]["type"], "assignment");
        assert_eq!(result[1]["left"]["name"], "name");
        let second_value = &result[1]["right"];
        assert_eq!(second_value["base"], "inputs");
        let path = second_value["path"].as_array().unwrap();
        assert_eq!(path[0]["type"], "dot");
        assert_eq!(path[0]["value"], "items");
        assert_eq!(path[1]["type"], "index");
        assert_eq!(path[1]["value"], 1);
        assert_eq!(path[2]["type"], "dot");
        assert_eq!(path[2]["value"], "name");
    }

    #[test]
    fn test_bracket_notation_string_key() {
        let workflow = r#"
workflow(ctx, inputs) {
  let val = data["key"]
  let nested = obj["outer"]["inner"]
}
        "#;
        let result = parse_workflow(workflow).unwrap();
        assert_eq!(result.len(), 2);

        // First: data["key"]
        let first_value = &result[0]["right"];
        assert_eq!(first_value["base"], "data");
        let path = first_value["path"].as_array().unwrap();
        assert_eq!(path[0]["type"], "bracket");
        assert_eq!(path[0]["value"], "key");

        // Second: obj["outer"]["inner"]
        let second_value = &result[1]["right"];
        assert_eq!(second_value["base"], "obj");
        let path = second_value["path"].as_array().unwrap();
        assert_eq!(path[0]["type"], "bracket");
        assert_eq!(path[0]["value"], "outer");
        assert_eq!(path[1]["type"], "bracket");
        assert_eq!(path[1]["value"], "inner");
    }

    #[test]
    fn test_bracket_notation_variable_index() {
        let workflow = r#"
workflow(ctx, inputs) {
  let i = 0
  let item = data[i]
  let field = obj[keyName]
}
        "#;
        let result = parse_workflow(workflow).unwrap();
        assert_eq!(result.len(), 3);

        // Second: data[i]
        let second_value = &result[1]["right"];
        assert_eq!(second_value["base"], "data");
        let path = second_value["path"].as_array().unwrap();
        assert_eq!(path[0]["type"], "bracket_var");
        assert_eq!(path[0]["value"], "i");

        // Third: obj[keyName]
        let third_value = &result[2]["right"];
        assert_eq!(third_value["base"], "obj");
        let path = third_value["path"].as_array().unwrap();
        assert_eq!(path[0]["type"], "bracket_var");
        assert_eq!(path[0]["value"], "keyName");
    }

    #[test]
    fn test_bracket_notation_mixed() {
        let workflow = r#"
workflow(ctx, inputs) {
  let val = data.items[0].fields["name"]
}
        "#;
        let result = parse_workflow(workflow).unwrap();
        assert_eq!(result.len(), 1);

        let value = &result[0]["right"];
        assert_eq!(value["base"], "data");
        let path = value["path"].as_array().unwrap();
        assert_eq!(path.len(), 4);
        assert_eq!(path[0]["type"], "dot");
        assert_eq!(path[0]["value"], "items");
        assert_eq!(path[1]["type"], "index");
        assert_eq!(path[1]["value"], 0);
        assert_eq!(path[2]["type"], "dot");
        assert_eq!(path[2]["value"], "fields");
        assert_eq!(path[3]["type"], "bracket");
        assert_eq!(path[3]["value"], "name");
    }
}
