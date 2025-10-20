use serde_json::{json, Value as JsonValue};

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    UnexpectedToken { line: usize, message: String },
    InvalidJson { line: usize, message: String },
    UnknownFunction { line: usize, function: String },
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
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a .crnt workflow script into a JSON array of steps
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
    let mut steps = Vec::new();

    for (line_num, line) in source.lines().enumerate() {
        let line = line.trim();
        let line_number = line_num + 1; // 1-indexed for user-facing errors

        // Skip empty lines
        if line.is_empty() {
            continue;
        }

        // Parse task() calls
        if line.starts_with("task(") {
            let step = parse_task_call(line, line_number)?;
            steps.push(step);
        }
        // Parse sleep() calls
        else if line.starts_with("sleep(") {
            let step = parse_sleep_call(line, line_number)?;
            steps.push(step);
        }
        // Unknown function
        else {
            // Try to extract function name for better error message
            if let Some(paren_pos) = line.find('(') {
                let func_name = &line[..paren_pos];
                return Err(ParseError::UnknownFunction {
                    line: line_number,
                    function: func_name.to_string(),
                });
            } else {
                return Err(ParseError::UnexpectedToken {
                    line: line_number,
                    message: format!("Expected function call, got: {}", line),
                });
            }
        }
    }

    Ok(steps)
}

/// Parse a task() call
/// Format: task("name") or task("name", {...})
fn parse_task_call(line: &str, line_number: usize) -> Result<JsonValue, ParseError> {
    // Remove "task(" prefix and ")" suffix
    let content = line.strip_prefix("task(")
        .and_then(|s| s.strip_suffix(")"))
        .ok_or_else(|| ParseError::UnexpectedToken {
            line: line_number,
            message: "Expected task(...) with closing parenthesis".to_string(),
        })?;

    // Split by comma to get arguments
    // This is naive - doesn't handle commas inside JSON objects properly
    // But for MVP, we'll use a simple approach
    let parts = split_arguments(content);

    if parts.is_empty() {
        return Err(ParseError::UnexpectedToken {
            line: line_number,
            message: "task() requires at least one argument (task name)".to_string(),
        });
    }

    // First argument is the task name (must be a string)
    let task_name = parse_string_literal(&parts[0], line_number)?;

    // Second argument (if present) is the inputs object
    let inputs = if parts.len() > 1 {
        parse_json_object(&parts[1], line_number)?
    } else {
        json!({})
    };

    Ok(json!({
        "type": "task",
        "task": task_name,
        "inputs": inputs
    }))
}

/// Parse a sleep() call
/// Format: sleep(10)
fn parse_sleep_call(line: &str, line_number: usize) -> Result<JsonValue, ParseError> {
    // Remove "sleep(" prefix and ")" suffix
    let content = line.strip_prefix("sleep(")
        .and_then(|s| s.strip_suffix(")"))
        .ok_or_else(|| ParseError::UnexpectedToken {
            line: line_number,
            message: "Expected sleep(...) with closing parenthesis".to_string(),
        })?;

    // Parse the duration as a number
    let duration: u64 = content.trim().parse()
        .map_err(|_| ParseError::UnexpectedToken {
            line: line_number,
            message: format!("Expected number for sleep duration, got: {}", content),
        })?;

    Ok(json!({
        "type": "sleep",
        "duration": duration
    }))
}

/// Split function arguments by comma
/// This is a naive implementation that doesn't handle nested commas in JSON objects
/// For MVP, we'll assume simple usage
fn split_arguments(content: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut brace_depth = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for ch in content.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }

        match ch {
            '\\' if in_string => {
                escape_next = true;
                current.push(ch);
            }
            '"' | '\'' => {
                in_string = !in_string;
                current.push(ch);
            }
            '{' if !in_string => {
                brace_depth += 1;
                current.push(ch);
            }
            '}' if !in_string => {
                brace_depth -= 1;
                current.push(ch);
            }
            ',' if !in_string && brace_depth == 0 => {
                parts.push(current.trim().to_string());
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }

    parts
}

/// Parse a string literal (removes quotes)
fn parse_string_literal(s: &str, line_number: usize) -> Result<String, ParseError> {
    let s = s.trim();

    // Check for double quotes
    if s.starts_with('"') && s.ends_with('"') {
        return Ok(s[1..s.len()-1].to_string());
    }

    // Check for single quotes
    if s.starts_with('\'') && s.ends_with('\'') {
        return Ok(s[1..s.len()-1].to_string());
    }

    Err(ParseError::UnexpectedToken {
        line: line_number,
        message: format!("Expected string literal (in quotes), got: {}", s),
    })
}

/// Parse a JSON object
fn parse_json_object(s: &str, line_number: usize) -> Result<JsonValue, ParseError> {
    let s = s.trim();

    serde_json::from_str(s)
        .map_err(|e| ParseError::InvalidJson {
            line: line_number,
            message: format!("{}", e),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_workflow() {
        let source = r#"
task("do-something", { "hey": "hello" })
sleep(10)
task("do-another")
        "#;

        let result = parse_workflow(source).unwrap();

        assert_eq!(result.len(), 3);

        assert_eq!(result[0]["type"], "task");
        assert_eq!(result[0]["task"], "do-something");
        assert_eq!(result[0]["inputs"]["hey"], "hello");

        assert_eq!(result[1]["type"], "sleep");
        assert_eq!(result[1]["duration"], 10);

        assert_eq!(result[2]["type"], "task");
        assert_eq!(result[2]["task"], "do-another");
        assert_eq!(result[2]["inputs"], json!({}));
    }

    #[test]
    fn test_parse_task_without_inputs() {
        let source = r#"task("my-task")"#;
        let result = parse_workflow(source).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["inputs"], json!({}));
    }

    #[test]
    fn test_parse_empty_lines() {
        let source = r#"
task("first")

task("second")

        "#;
        let result = parse_workflow(source).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_error_unknown_function() {
        let source = r#"unknown_func(123)"#;
        let result = parse_workflow(source);

        assert!(result.is_err());
        match result.unwrap_err() {
            ParseError::UnknownFunction { line, function } => {
                assert_eq!(line, 1);
                assert_eq!(function, "unknown_func");
            }
            _ => panic!("Expected UnknownFunction error"),
        }
    }

    #[test]
    fn test_parse_error_missing_closing_paren() {
        let source = r#"task("foo""#;
        let result = parse_workflow(source);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_error_invalid_json() {
        let source = r#"task("foo", {invalid})"#;
        let result = parse_workflow(source);

        assert!(result.is_err());
        match result.unwrap_err() {
            ParseError::InvalidJson { .. } => {},
            _ => panic!("Expected InvalidJson error"),
        }
    }

    #[test]
    fn test_parse_sleep_non_numeric() {
        let source = r#"sleep("not a number")"#;
        let result = parse_workflow(source);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_single_quotes() {
        let source = r#"task('my-task')"#;
        let result = parse_workflow(source).unwrap();

        assert_eq!(result[0]["task"], "my-task");
    }
}
