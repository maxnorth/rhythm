use pest::Parser;
use pest_derive::Parser;
use serde_json::{json, Value as JsonValue};

#[derive(Parser)]
#[grammar = "interpreter/workflow.pest"]
struct WorkflowParser;

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    UnexpectedToken { line: usize, message: String },
    InvalidJson { line: usize, message: String },
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
/// task("do-something", { "hey": "hello" })
/// sleep(10)
/// task("do-another")
/// ```
///
/// Output:
/// ```json
/// [
///   { "type": "task", "task": "do-something", "inputs": { "hey": "hello" } },
///   { "type": "sleep", "duration": 10 },
///   { "type": "task", "task": "do-another", "inputs": {} }
/// ]
/// ```
pub fn parse_workflow(source: &str) -> Result<Vec<JsonValue>, ParseError> {
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
            match inner_statement.as_rule() {
                Rule::assignment => {
                    steps.push(parse_assignment(inner_statement)?);
                }
                Rule::await_statement | Rule::await_task => {
                    steps.push(parse_await_statement(inner_statement)?);
                }
                Rule::task_call => {
                    steps.push(parse_task_call(inner_statement, false)?);
                }
                Rule::sleep_call => {
                    steps.push(parse_sleep_call(inner_statement, false)?);
                }
                _ => {}
            }
        }
    }

    Ok(steps)
}

fn parse_assignment(pair: pest::iterators::Pair<Rule>) -> Result<JsonValue, ParseError> {
    let mut inner = pair.into_inner();

    // Get variable name (identifier)
    let var_name_pair = inner.next()
        .ok_or_else(|| ParseError::PestError("assignment requires variable name".to_string()))?;
    let var_name = var_name_pair.as_str().to_string();

    // Get the task expression (either await_statement or task_call)
    let task_expr = inner.next()
        .ok_or_else(|| ParseError::PestError("assignment requires task expression".to_string()))?;

    // Parse the task (will have await field set appropriately)
    let mut task_json = match task_expr.as_rule() {
        Rule::await_statement | Rule::await_task => parse_await_statement(task_expr)?,
        Rule::task_call => parse_task_call(task_expr, false)?,
        Rule::sleep_call => parse_sleep_call(task_expr, false)?,
        _ => return Err(ParseError::PestError("invalid assignment expression".to_string())),
    };

    // Add variable name to task JSON
    if let Some(obj) = task_json.as_object_mut() {
        obj.insert("assign_to".to_string(), json!(var_name));
    }

    Ok(task_json)
}

fn parse_await_statement(pair: pest::iterators::Pair<Rule>) -> Result<JsonValue, ParseError> {
    let mut inner = pair.into_inner();

    // The inner element can be either task_call or sleep_call
    let call_pair = inner.next()
        .ok_or_else(|| ParseError::PestError("await requires a task call or sleep call".to_string()))?;

    // Parse with await=true
    match call_pair.as_rule() {
        Rule::task_call => parse_task_call(call_pair, true),
        Rule::sleep_call => parse_sleep_call(call_pair, true),
        _ => Err(ParseError::PestError("await can only be used with task() or sleep()".to_string())),
    }
}

fn parse_task_call(pair: pest::iterators::Pair<Rule>, await_completion: bool) -> Result<JsonValue, ParseError> {
    let line = pair.as_span().start_pos().line_col().0;
    let mut inner = pair.into_inner();

    // First element is the task name (string)
    let task_name_pair = inner.next()
        .ok_or_else(|| ParseError::UnexpectedToken {
            line,
            message: "task() requires a task name".to_string(),
        })?;

    let task_name = parse_string(task_name_pair)?;

    // Second element (if present) is the inputs object
    let inputs = if let Some(json_pair) = inner.next() {
        parse_json_object(json_pair, line)?
    } else {
        json!({})
    };

    Ok(json!({
        "type": "task",
        "task": task_name,
        "inputs": inputs,
        "await": await_completion
    }))
}

fn parse_sleep_call(pair: pest::iterators::Pair<Rule>, await_completion: bool) -> Result<JsonValue, ParseError> {
    let line = pair.as_span().start_pos().line_col().0;
    let mut inner = pair.into_inner();

    // Get the duration (integer)
    let duration_pair = inner.next()
        .ok_or_else(|| ParseError::UnexpectedToken {
            line,
            message: "sleep() requires a duration".to_string(),
        })?;

    let duration_str = duration_pair.as_str();
    let duration: u64 = duration_str.parse()
        .map_err(|_| ParseError::UnexpectedToken {
            line,
            message: format!("Expected integer for sleep duration, got: {}", duration_str),
        })?;

    Ok(json!({
        "type": "sleep",
        "duration": duration,
        "await": await_completion
    }))
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

fn parse_json_object(pair: pest::iterators::Pair<Rule>, line: usize) -> Result<JsonValue, ParseError> {
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
            let value = parse_json_value(value_pair, line)?;

            obj.insert(key, value);
        }
    }

    Ok(JsonValue::Object(obj))
}

fn parse_json_value(pair: pest::iterators::Pair<Rule>, line: usize) -> Result<JsonValue, ParseError> {
    let inner = pair.into_inner().next()
        .ok_or_else(|| ParseError::InvalidJson {
            line,
            message: "Empty JSON value".to_string(),
        })?;

    match inner.as_rule() {
        Rule::json_object => parse_json_object(inner, line),
        Rule::json_array => parse_json_array(inner, line),
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
            // This is a member access like inputs.userId or ctx.workflowId
            // Store as-is without $ prefix (unlike bare identifiers)
            let member_str = inner.as_str();
            Ok(JsonValue::String(member_str.to_string()))
        }
        Rule::identifier => {
            // This is a variable reference - add $ prefix in JSON to distinguish from literal strings
            // In .flow files, users write: task("foo", { "key": myVar })
            // Parser outputs JSON: { "key": "$myVar" } so executor knows it's a variable
            let var_name = inner.as_str();
            Ok(JsonValue::String(format!("${}", var_name)))
        }
        _ => Err(ParseError::InvalidJson {
            line,
            message: format!("Unexpected JSON value type: {:?}", inner.as_rule()),
        }),
    }
}

fn parse_json_array(pair: pest::iterators::Pair<Rule>, line: usize) -> Result<JsonValue, ParseError> {
    let mut arr = Vec::new();

    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::json_value {
            arr.push(parse_json_value(inner, line)?);
        }
    }

    Ok(JsonValue::Array(arr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_workflow() {
        let source = r#"
workflow(ctx, inputs) {
  task("do-something", { "hey": "hello" })
  sleep(10)
  task("do-another")
}
                "#;

        let result = parse_workflow(source).unwrap();

        assert_eq!(result.len(), 3);

        assert_eq!(result[0]["type"], "task");
        assert_eq!(result[0]["task"], "do-something");
        assert_eq!(result[0]["inputs"]["hey"], "hello");
        assert_eq!(result[0]["await"], false);

        assert_eq!(result[1]["type"], "sleep");
        assert_eq!(result[1]["duration"], 10);

        assert_eq!(result[2]["type"], "task");
        assert_eq!(result[2]["task"], "do-another");
        assert_eq!(result[2]["inputs"], json!({}));
        assert_eq!(result[2]["await"], false);
    }

    #[test]
    fn test_parse_await_task() {
        let source = r#"
workflow(ctx, inputs) {
  await task("fetch-data", { "id": 123 })
  task("log", { "msg": "fired and forgotten" })
}
                "#;

        let result = parse_workflow(source).unwrap();

        assert_eq!(result.len(), 2);

        // First task should have await=true
        assert_eq!(result[0]["type"], "task");
        assert_eq!(result[0]["task"], "fetch-data");
        assert_eq!(result[0]["inputs"]["id"], 123);
        assert_eq!(result[0]["await"], true);

        // Second task should have await=false
        assert_eq!(result[1]["type"], "task");
        assert_eq!(result[1]["task"], "log");
        assert_eq!(result[1]["await"], false);
    }

    #[test]
    fn test_parse_task_without_inputs() {
        let source = r#"workflow(ctx, inputs) { task("my-task") }"#;
        let result = parse_workflow(source).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["inputs"], json!({}));
    }

    #[test]
    fn test_parse_empty_lines() {
        let source = r#"
workflow(ctx, inputs) {
  task("first")

  task("second")

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
        let source = r#"workflow(ctx, inputs) { task("foo" }"#;
        let result = parse_workflow(source);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_error_invalid_json() {
        // Test invalid JSON syntax (missing closing brace)
        let source = r#"workflow(ctx, inputs) { task("foo", { "key": "value" ) }"#;
        let result = parse_workflow(source);

        assert!(result.is_err());
        // Could be either InvalidJson or PestError depending on where it fails
    }

    #[test]
    fn test_parse_sleep_non_numeric() {
        let source = r#"workflow(ctx, inputs) { sleep("not a number") }"#;
        let result = parse_workflow(source);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_single_quotes() {
        let source = r#"workflow(ctx, inputs) { task('my-task') }"#;
        let result = parse_workflow(source).unwrap();

        assert_eq!(result[0]["task"], "my-task");
    }

    #[test]
    fn test_parse_comments() {
        let source = r#"
workflow(ctx, inputs) {
  // This is a comment
  task("first")
  // Another comment
  sleep(5)
}
                "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_nested_json() {
        let source = r#"workflow(ctx, inputs) { task("process", { "user": { "name": "Alice", "age": 30 }, "count": 5 }) }"#;
        let result = parse_workflow(source).unwrap();

        assert_eq!(result[0]["inputs"]["user"]["name"], "Alice");
        assert_eq!(result[0]["inputs"]["user"]["age"], 30);
        assert_eq!(result[0]["inputs"]["count"], 5);
    }

    #[test]
    fn test_parse_json_with_commas() {
        let source = r#"workflow(ctx, inputs) { task("send", { "to": "user@example.com", "subject": "Hello", "body": "World" }) }"#;
        let result = parse_workflow(source).unwrap();

        assert_eq!(result[0]["inputs"]["to"], "user@example.com");
        assert_eq!(result[0]["inputs"]["subject"], "Hello");
        assert_eq!(result[0]["inputs"]["body"], "World");
    }

    #[test]
    fn test_parse_variable_assignment() {
        let source = r#"
workflow(ctx, inputs) {
  let order_id = await task("create_order", { "amount": 100 })
  let result = task("log", { "msg": "test" })
}
                "#;

        let result = parse_workflow(source).unwrap();

        assert_eq!(result.len(), 2);

        // First assignment: with await
        assert_eq!(result[0]["type"], "task");
        assert_eq!(result[0]["task"], "create_order");
        assert_eq!(result[0]["inputs"]["amount"], 100);
        assert_eq!(result[0]["await"], true);
        assert_eq!(result[0]["assign_to"], "order_id");

        // Second assignment: without await
        assert_eq!(result[1]["type"], "task");
        assert_eq!(result[1]["task"], "log");
        assert_eq!(result[1]["await"], false);
        assert_eq!(result[1]["assign_to"], "result");
    }

    #[test]
    fn test_parse_variable_names() {
        let source = r#"
workflow(ctx, inputs) {
  let my_var = task("test")
  let _private = task("test")
  let var123 = task("test")
}
                "#;

        let result = parse_workflow(source).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0]["assign_to"], "my_var");
        assert_eq!(result[1]["assign_to"], "_private");
        assert_eq!(result[2]["assign_to"], "var123");
    }

    #[test]
    fn test_parse_variable_references() {
        // Test bare identifier variable references in JSON
        let source = r#"
workflow(ctx, inputs) {
  let order_id = await task("create_order", { "amount": 100 })
  await task("charge", { "order_id": order_id, "amount": 50 })
}
                "#;

        let result = parse_workflow(source).unwrap();

        assert_eq!(result.len(), 2);

        // First task creates order_id
        assert_eq!(result[0]["assign_to"], "order_id");
        assert_eq!(result[0]["inputs"]["amount"], 100);

        // Second task uses order_id as a variable reference
        // Parser adds $ prefix to distinguish from literal strings
        assert_eq!(result[1]["inputs"]["order_id"], "$order_id");
        assert_eq!(result[1]["inputs"]["amount"], 50);
    }

    #[test]
    fn test_parse_json_all_types() {
        let source = r#"
workflow(ctx, inputs) {
  task("test", {
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

        assert_eq!(result.len(), 1);
        let inputs = &result[0]["inputs"];

        assert_eq!(inputs["string"], "hello");
        assert_eq!(inputs["number"], 42);
        assert_eq!(inputs["float"], 3.14);
        assert_eq!(inputs["bool_true"], true);
        assert_eq!(inputs["bool_false"], false);
        assert_eq!(inputs["null_val"], JsonValue::Null);
        assert_eq!(inputs["var_ref"], "$my_variable");
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
        let source = r#"workflow(ctx, inputs) { task("t", { "a": 1, }) }"#;
        let result = parse_workflow(source);
        println!("Trailing comma in object: {:?}", result);
        // This might fail - trailing commas aren't standard JSON
    }

    #[test]
    fn test_edge_trailing_comma_array() {
        let source = r#"workflow(ctx, inputs) { task("t", { "arr": [1, 2,] }) }"#;
        let result = parse_workflow(source);
        println!("Trailing comma in array: {:?}", result);
    }

    #[test]
    fn test_edge_empty_array() {
        let source = r#"workflow(ctx, inputs) { task("t", { "arr": [] }) }"#;
        let result = parse_workflow(source).unwrap();
        assert!(result[0]["inputs"]["arr"].is_array());
        assert_eq!(result[0]["inputs"]["arr"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_edge_array_only_variables() {
        let source = r#"workflow(ctx, inputs) { task("t", { "arr": [var1, var2, var3] }) }"#;
        let result = parse_workflow(source).unwrap();
        let arr = &result[0]["inputs"]["arr"];
        assert_eq!(arr[0], "$var1");
        assert_eq!(arr[1], "$var2");
        assert_eq!(arr[2], "$var3");
    }

    #[test]
    fn test_edge_deeply_nested_json() {
        let source = r#"workflow(ctx, inputs) { task("t", { "a": { "b": { "c": { "d": { "e": { "f": "deep" } } } } } }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["a"]["b"]["c"]["d"]["e"]["f"], "deep");
    }

    #[test]
    fn test_edge_scientific_notation() {
        let source = r#"workflow(ctx, inputs) { task("t", { "n1": 1e10, "n2": 1e-10, "n3": 1.5E+5 }) }"#;
        let result = parse_workflow(source).unwrap();
        let inputs = &result[0]["inputs"];
        // Check that scientific notation parses correctly
        assert!(inputs["n1"].is_number());
        assert!(inputs["n2"].is_number());
        assert!(inputs["n3"].is_number());
    }

    #[test]
    fn test_edge_negative_numbers() {
        let source = r#"workflow(ctx, inputs) { task("t", { "int": -42, "float": -3.14 }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["int"], -42);
        assert_eq!(result[0]["inputs"]["float"], -3.14);
    }

    #[test]
    fn test_edge_no_spaces() {
        let source = r#"workflow(ctx, inputs) { let x=task("t",{}) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["assign_to"], "x");
    }

    #[test]
    fn test_edge_unicode_task_name() {
        let source = r#"workflow(ctx, inputs) { task("😀", {}) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["task"], "😀");
    }

    #[test]
    fn test_edge_special_chars_in_task_name() {
        let source = r#"workflow(ctx, inputs) { task("my-task.name/v1:process", {}) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["task"], "my-task.name/v1:process");
    }

    #[test]
    fn test_edge_escaped_chars_in_strings() {
        let source = r#"workflow(ctx, inputs) { task("t", { "msg": "He said \"hello\"", "path": "C:\\Users" }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["msg"], r#"He said "hello""#);
        assert_eq!(result[0]["inputs"]["path"], r"C:\Users");
    }

    #[test]
    fn test_edge_variable_shadowing() {
        // Same variable name assigned twice - later should overwrite
        let source = r#"
workflow(ctx, inputs) {
  let x = task("t1", {})
  let x = task("t2", {})
  task("t3", { "val": x })
}
                "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0]["assign_to"], "x");
        assert_eq!(result[1]["assign_to"], "x");
        assert_eq!(result[2]["inputs"]["val"], "$x");
        // Note: executor should use the last assigned value
    }

    #[test]
    fn test_edge_empty_task_name() {
        let source = r#"workflow(ctx, inputs) { task("", {}) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["task"], "");
    }

    #[test]
    fn test_edge_empty_string_value() {
        let source = r#"workflow(ctx, inputs) { task("t", { "msg": "" }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["msg"], "");
    }

    #[test]
    fn test_edge_task_without_inputs() {
        // Task without second argument should work
        let source = r#"workflow(ctx, inputs) { task("task_name") }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["task"], "task_name");
        assert!(result[0]["inputs"].is_object());
    }

    #[test]
    fn test_edge_reserved_word_variable_names() {
        // Reserved words from other languages should work as variable names
        let source = r#"
workflow(ctx, inputs) {
  let class = task("t1", {})
  let import = task("t2", {})
  let return = task("t3", {})
}
                "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0]["assign_to"], "class");
        assert_eq!(result[1]["assign_to"], "import");
        assert_eq!(result[2]["assign_to"], "return");
    }

    #[test]
    fn test_edge_nested_empty_structures() {
        let source = r#"workflow(ctx, inputs) { task("t", { "a": {}, "b": [] }) }"#;
        let result = parse_workflow(source).unwrap();
        assert!(result[0]["inputs"]["a"].is_object());
        assert!(result[0]["inputs"]["b"].is_array());
        assert_eq!(result[0]["inputs"]["a"].as_object().unwrap().len(), 0);
        assert_eq!(result[0]["inputs"]["b"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_edge_mixed_quote_types() {
        let source = r#"workflow(ctx, inputs) { task("t", { "key1": 'value1', 'key2': "value2" }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["key1"], "value1");
        assert_eq!(result[0]["inputs"]["key2"], "value2");
    }

    #[test]
    fn test_edge_number_starting_with_dot() {
        // .5 should be valid (same as 0.5)
        let source = r#"workflow(ctx, inputs) { task("t", { "val": .5 }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["val"], 0.5);
    }

    #[test]
    fn test_edge_number_ending_with_dot() {
        // 5. should be valid (same as 5.0)
        let source = r#"workflow(ctx, inputs) { task("t", { "val": 5. }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["val"], 5.0);
    }

    #[test]
    fn test_edge_negative_number_with_dot() {
        // -.5 should work
        let source = r#"workflow(ctx, inputs) { task("t", { "val": -.5 }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["val"], -0.5);
    }

    #[test]
    fn test_edge_very_large_number() {
        let source = r#"workflow(ctx, inputs) { task("t", { "val": 999999999999999999999 }) }"#;
        let result = parse_workflow(source);
        // Large numbers might overflow or parse differently
        println!("Very large number: {:?}", result.as_ref().map(|v| &v[0]["inputs"]["val"]));
    }

    #[test]
    fn test_edge_zero_values() {
        let source = r#"workflow(ctx, inputs) { task("t", { "int": 0, "float": 0.0 }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["int"], 0);
        assert_eq!(result[0]["inputs"]["float"], 0.0);
    }

    #[test]
    fn test_edge_whitespace_only_object() {
        let source = r#"workflow(ctx, inputs) { task("t", {   }) }"#;
        let result = parse_workflow(source).unwrap();
        assert!(result[0]["inputs"].is_object());
        assert_eq!(result[0]["inputs"].as_object().unwrap().len(), 0);
    }

    #[test]
    fn test_edge_multiline_json() {
        let source = r#"
workflow(ctx, inputs) {
  task("t", {
    "key1": "value1",
    "key2": "value2"
  })
}
                "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["key1"], "value1");
        assert_eq!(result[0]["inputs"]["key2"], "value2");
    }

    #[test]
    fn test_edge_consecutive_commas() {
        // Double comma should fail
        let source = r#"workflow(ctx, inputs) { task("t", { "a": 1,, "b": 2 }) }"#;
        let result = parse_workflow(source);
        assert!(result.is_err(), "Consecutive commas should be invalid");
    }

    #[test]
    fn test_edge_unquoted_variable_value() {
        // Variable as value (unquoted identifier) should work
        let source = r#"workflow(ctx, inputs) { task("t", { "val": my_var }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["val"], "$my_var");
    }

    #[test]
    fn test_edge_tabs_as_whitespace() {
        let source = "workflow(ctx, inputs) { let\tx\t=\ttask(\"t\",\t{}) }";
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["assign_to"], "x");
    }

    #[test]
    fn test_edge_trailing_comment() {
        let source = r#"
workflow(ctx, inputs) {
  task("t", {}) // trailing comment
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_edge_string_with_escapes() {
        let source = r#"workflow(ctx, inputs) { task("t", { "msg": "\n\t\r" }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["msg"], "\n\t\r");
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
        let source = r#"workflow(ctx, inputs) { task("t1", {}) # this should fail }"#;
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
        let source = r#"workflow(ctx, inputs) { let x = task("t1", {}) let y = task("t2", {}) }"#;
        let result = parse_workflow(source);
        assert!(result.is_err(), "Multiple statements on one line should fail");
    }

    #[test]
    fn test_edge_semicolons_disallowed() {
        // Semicolons should be explicitly disallowed (unlike JS)
        let source = r#"workflow(ctx, inputs) { task("t1", {}); }"#;
        let result = parse_workflow(source);
        assert!(result.is_err(), "Semicolons should not be allowed");

        // Check error message
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Semicolons are not allowed"), "Error message should mention semicolons");
    }

    #[test]
    fn test_edge_multiple_statements_with_semicolons() {
        // Multiple statements separated by semicolons should fail
        let source = r#"workflow(ctx, inputs) { task("t1", {}); task("t2", {}) }"#;
        let result = parse_workflow(source);
        assert!(result.is_err(), "Semicolon-separated statements should fail");
    }

    #[test]
    fn test_edge_hex_numbers() {
        // Hex numbers - now supported!
        let source = r#"workflow(ctx, inputs) { task("t", { "val": 0x1234, "color": 0xFF00FF }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["val"], 0x1234);  // 4660
        assert_eq!(result[0]["inputs"]["color"], 0xFF00FF);  // 16711935
    }

    #[test]
    fn test_edge_binary_numbers() {
        // Binary numbers - now supported!
        let source = r#"workflow(ctx, inputs) { task("t", { "flags": 0b1010, "mask": 0b11110000 }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["flags"], 0b1010);  // 10
        assert_eq!(result[0]["inputs"]["mask"], 0b11110000);  // 240
    }

    #[test]
    fn test_edge_numbers_with_underscores() {
        // Numbers with underscores - now supported for readability!
        let source = r#"workflow(ctx, inputs) { task("t", { "big": 1_000_000, "pi": 3.14_15_92 }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["big"], 1_000_000);
        assert_eq!(result[0]["inputs"]["pi"], 3.141592);
    }

    #[test]
    fn test_edge_hex_with_underscores() {
        // Combine hex and underscores
        let source = r#"workflow(ctx, inputs) { task("t", { "addr": 0xDEAD_BEEF }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["addr"], 3735928559i64);
    }

    #[test]
    fn test_edge_negative_hex() {
        // Negative hex numbers
        let source = r#"workflow(ctx, inputs) { task("t", { "val": -0xFF }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["val"], -255);
    }

    #[test]
    fn test_edge_unquoted_keys() {
        // Unquoted keys (JSON5 style)
        let source = r#"workflow(ctx, inputs) { task("t", { name: "Alice", age: 30, active: true }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["name"], "Alice");
        assert_eq!(result[0]["inputs"]["age"], 30);
        assert_eq!(result[0]["inputs"]["active"], true);
    }

    #[test]
    fn test_edge_mixed_quoted_unquoted_keys() {
        // Mix quoted and unquoted keys
        let source = r#"workflow(ctx, inputs) { task("t", { "user-id": 123, name: "Alice", "is-admin": false }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["user-id"], 123);
        assert_eq!(result[0]["inputs"]["name"], "Alice");
        assert_eq!(result[0]["inputs"]["is-admin"], false);
    }

    #[test]
    fn test_edge_unquoted_keys_with_variables() {
        // Unquoted keys with variable values
        let source = r#"workflow(ctx, inputs) { task("t", { userId: user_id, config: my_config }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["userId"], "$user_id");
        assert_eq!(result[0]["inputs"]["config"], "$my_config");
    }

    #[test]
    fn test_edge_nested_unquoted_keys() {
        // Nested objects with unquoted keys
        let source = r#"workflow(ctx, inputs) { task("t", { user: { name: "Alice", age: 30 }, count: 5 }) }"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result[0]["inputs"]["user"]["name"], "Alice");
        assert_eq!(result[0]["inputs"]["user"]["age"], 30);
        assert_eq!(result[0]["inputs"]["count"], 5);
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
        // Test inputs.fieldName syntax (no $ prefix)
        let source = r#"
workflow(ctx, inputs) {
  await task("process", { userId: inputs.userId, orderId: inputs.orderId })
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        // Member access should NOT have $ prefix (unlike bare identifiers)
        assert_eq!(result[0]["inputs"]["userId"], "inputs.userId");
        assert_eq!(result[0]["inputs"]["orderId"], "inputs.orderId");
    }

    #[test]
    fn test_ctx_member_access() {
        // Test ctx.workflowId syntax
        let source = r#"
workflow(ctx, inputs) {
  await task("log", { workflowId: ctx.workflowId })
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        // ctx.workflowId should be stored as-is
        assert_eq!(result[0]["inputs"]["workflowId"], "ctx.workflowId");
    }

    #[test]
    fn test_mixed_member_access_and_variables() {
        // Test mixing inputs.*, ctx.*, and bare variable references
        let source = r#"
workflow(ctx, inputs) {
  let result = await task("validate", { userId: inputs.userId })
  await task("process", {
    validationResult: result,
    workflow: ctx.workflowId,
    order: inputs.orderId
  })
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 2);

        // First statement: task using inputs.userId
        assert_eq!(result[0]["inputs"]["userId"], "inputs.userId");  // Member access

        // Second statement: mixed references
        assert_eq!(result[1]["inputs"]["validationResult"], "$result");  // Bare variable reference
        assert_eq!(result[1]["inputs"]["workflow"], "ctx.workflowId");  // Member access
        assert_eq!(result[1]["inputs"]["order"], "inputs.orderId");  // Member access
    }

    #[test]
    fn test_await_sleep() {
        // Test await sleep() syntax
        let source = r#"
workflow(ctx, inputs) {
  await task("start", {})
  await sleep(5)
  await task("finish", {})
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 3);

        // First task
        assert_eq!(result[0]["type"], "task");
        assert_eq!(result[0]["await"], true);

        // Sleep with await
        assert_eq!(result[1]["type"], "sleep");
        assert_eq!(result[1]["duration"], 5);
        assert_eq!(result[1]["await"], true);

        // Last task
        assert_eq!(result[2]["type"], "task");
        assert_eq!(result[2]["await"], true);
    }

    #[test]
    fn test_dollar_sign_escape() {
        // Test that $$ in literal strings becomes a single $
        let source = r#"workflow(ctx, inputs) {
  task("test", { "price": "$$99.99", "note": "Only $$50!" })
}"#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        // $$ should be unescaped to single $
        assert_eq!(result[0]["inputs"]["price"], "$99.99");
        assert_eq!(result[0]["inputs"]["note"], "Only $50!");
    }

    #[test]
    fn test_sleep_without_await() {
        // Test that sleep() without await has await=false
        let source = r#"
workflow(ctx, inputs) {
  sleep(10)
}
        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 1);

        assert_eq!(result[0]["type"], "sleep");
        assert_eq!(result[0]["duration"], 10);
        assert_eq!(result[0]["await"], false);
    }
}
