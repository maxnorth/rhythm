use tower_lsp::lsp_types::*;

use crate::hover::{get_hover, get_hover_from_ast};
use crate::parser::parse_workflow;

fn extract_hover_text(hover: &Hover) -> &str {
    match &hover.contents {
        HoverContents::Markup(markup) => &markup.value,
        _ => panic!("Expected markup content"),
    }
}

// =============================================================================
// get_hover - keyword tests
// =============================================================================

#[test]
fn test_hover_keyword_let() {
    let source = "let x = 42";
    let hover = get_hover(source, 0, 1).expect("Should return hover for 'let'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**let**"));
    assert!(text.contains("(keyword)"));
}

#[test]
fn test_hover_keyword_const() {
    let source = "const x = 42";
    let hover = get_hover(source, 0, 2).expect("Should return hover for 'const'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**const**"));
    assert!(text.contains("(keyword)"));
}

#[test]
fn test_hover_keyword_if() {
    let source = "if (true) { }";
    let hover = get_hover(source, 0, 1).expect("Should return hover for 'if'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**if**"));
}

#[test]
fn test_hover_keyword_return() {
    let source = "return 42";
    let hover = get_hover(source, 0, 3).expect("Should return hover for 'return'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**return**"));
}

#[test]
fn test_hover_keyword_true() {
    let source = "let x = true";
    let hover = get_hover(source, 0, 9).expect("Should return hover for 'true'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**true**"));
}

#[test]
fn test_hover_keyword_false() {
    let source = "let x = false";
    let hover = get_hover(source, 0, 10).expect("Should return hover for 'false'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**false**"));
}

#[test]
fn test_hover_keyword_null() {
    let source = "let x = null";
    let hover = get_hover(source, 0, 9).expect("Should return hover for 'null'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**null**"));
}

// =============================================================================
// get_hover - builtin module tests
// =============================================================================

#[test]
fn test_hover_module_task() {
    let source = "Task.run()";
    let hover = get_hover(source, 0, 2).expect("Should return hover for 'Task'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**Task**"));
    assert!(text.contains("(module)"));
    assert!(text.contains("**Methods:**"));
}

#[test]
fn test_hover_module_timer() {
    let source = "Timer.delay(5)";
    let hover = get_hover(source, 0, 2).expect("Should return hover for 'Timer'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**Timer**"));
    assert!(text.contains("(module)"));
}

#[test]
fn test_hover_module_signal() {
    let source = "Signal.next()";
    let hover = get_hover(source, 0, 3).expect("Should return hover for 'Signal'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**Signal**"));
}

#[test]
fn test_hover_module_workflow() {
    let source = "Workflow.run()";
    let hover = get_hover(source, 0, 4).expect("Should return hover for 'Workflow'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**Workflow**"));
}

#[test]
fn test_hover_module_promise() {
    let source = "Promise.all([])";
    let hover = get_hover(source, 0, 4).expect("Should return hover for 'Promise'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**Promise**"));
}

#[test]
fn test_hover_module_math() {
    let source = "Math.floor(3.5)";
    let hover = get_hover(source, 0, 2).expect("Should return hover for 'Math'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**Math**"));
}

#[test]
fn test_hover_module_inputs() {
    let source = "Inputs.foo";
    let hover = get_hover(source, 0, 3).expect("Should return hover for 'Inputs'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**Inputs**"));
}

// =============================================================================
// get_hover - module method tests
// =============================================================================

#[test]
fn test_hover_method_task_run() {
    let source = "Task.run()";
    // Position cursor at 'run' (after the dot)
    let hover = get_hover(source, 0, 6).expect("Should return hover for 'Task.run'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("Task.run"));
    assert!(text.contains("taskName"));
}

#[test]
fn test_hover_method_timer_delay() {
    let source = "Timer.delay(5)";
    let hover = get_hover(source, 0, 8).expect("Should return hover for 'Timer.delay'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("Timer.delay"));
    assert!(text.contains("seconds"));
}

#[test]
fn test_hover_method_promise_all() {
    let source = "Promise.all([])";
    let hover = get_hover(source, 0, 9).expect("Should return hover for 'Promise.all'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("Promise.all"));
}

#[test]
fn test_hover_method_promise_any() {
    let source = "Promise.any([])";
    let hover = get_hover(source, 0, 9).expect("Should return hover for 'Promise.any'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("Promise.any"));
}

#[test]
fn test_hover_method_promise_race() {
    let source = "Promise.race([])";
    let hover = get_hover(source, 0, 10).expect("Should return hover for 'Promise.race'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("Promise.race"));
}

// =============================================================================
// get_hover - array/string method tests
// =============================================================================

#[test]
fn test_hover_array_length() {
    let source = "arr.length()";
    let hover = get_hover(source, 0, 6).expect("Should return hover for 'length'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("length"));
}

#[test]
fn test_hover_string_split() {
    let source = "str.split(',')";
    let hover = get_hover(source, 0, 6).expect("Should return hover for 'split'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("split"));
}

// =============================================================================
// get_hover - edge cases
// =============================================================================

#[test]
fn test_hover_no_match() {
    let source = "let myVariable = 42";
    // Hover over 'myVariable' which isn't a keyword or builtin
    let result = get_hover(source, 0, 6);
    assert!(result.is_none());
}

#[test]
fn test_hover_empty_position() {
    let source = "let x = 42";
    // Position beyond the line
    let result = get_hover(source, 0, 100);
    assert!(result.is_none());
}

#[test]
fn test_hover_invalid_line() {
    let source = "let x = 42";
    let result = get_hover(source, 10, 0);
    assert!(result.is_none());
}

#[test]
fn test_hover_multiline() {
    let source = "let x = 1\nlet y = 2\nreturn x";
    // Hover over 'return' on line 2 (0-indexed)
    let hover = get_hover(source, 2, 3).expect("Should return hover for 'return'");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**return**"));
}

// =============================================================================
// get_hover_from_ast tests
// =============================================================================

#[test]
fn test_hover_from_ast_variable() {
    let source = "let myVar = 42\nreturn myVar";
    let workflow = parse_workflow(source).expect("Should parse");

    // Hover over 'myVar' on return line - should show as variable
    let hover = get_hover_from_ast(&workflow, source, 1, 8).expect("Should return hover");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**myVar**"));
    assert!(text.contains("(variable)"));
}

#[test]
fn test_hover_from_ast_number_literal() {
    let source = "let x = 42";
    let workflow = parse_workflow(source).expect("Should parse");

    // Hover over the number literal
    let hover = get_hover_from_ast(&workflow, source, 0, 8).expect("Should return hover");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**number**"));
    assert!(text.contains("42"));
}

#[test]
fn test_hover_from_ast_string_literal() {
    let source = r#"let x = "hello""#;
    let workflow = parse_workflow(source).expect("Should parse");

    // Hover over the string literal
    let hover = get_hover_from_ast(&workflow, source, 0, 10).expect("Should return hover");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**string**"));
    assert!(text.contains("hello"));
}

#[test]
fn test_hover_from_ast_boolean_literal() {
    let source = "let x = true";
    let workflow = parse_workflow(source).expect("Should parse");

    // Hover over 'true' - this should match keyword first
    let hover = get_hover_from_ast(&workflow, source, 0, 9).expect("Should return hover");
    let text = extract_hover_text(&hover);
    // Keywords take precedence, so this shows as keyword
    assert!(text.contains("**true**"));
}

#[test]
fn test_hover_from_ast_null_literal() {
    let source = "let x = null";
    let workflow = parse_workflow(source).expect("Should parse");

    let hover = get_hover_from_ast(&workflow, source, 0, 9).expect("Should return hover");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**null**"));
}

#[test]
fn test_hover_from_ast_builtin_module() {
    let source = "Task.run()";
    let workflow = parse_workflow(source).expect("Should parse");

    // Hover over 'Task' - should show module info
    let hover = get_hover_from_ast(&workflow, source, 0, 2).expect("Should return hover");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**Task**"));
    assert!(text.contains("(module)"));
}

#[test]
fn test_hover_from_ast_list_element() {
    let source = "let x = [1, 2, 3]";
    let workflow = parse_workflow(source).expect("Should parse");

    // Hover over '1' in the list
    let hover = get_hover_from_ast(&workflow, source, 0, 9).expect("Should return hover");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**number**"));
}

#[test]
fn test_hover_from_ast_in_if_statement() {
    let source = "if (true) {\n  let x = 42\n}";
    let workflow = parse_workflow(source).expect("Should parse");

    // Hover over '42' inside the if block
    let hover = get_hover_from_ast(&workflow, source, 1, 12).expect("Should return hover");
    let text = extract_hover_text(&hover);
    assert!(text.contains("**number**"));
}
