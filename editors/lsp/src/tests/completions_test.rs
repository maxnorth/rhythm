use tower_lsp::lsp_types::CompletionItemKind;

use crate::completions::{
    collect_variables, get_completions, get_signature_help, CompletionContext,
};
use crate::parser::parse_workflow;

// =============================================================================
// CompletionContext::from_position tests
// =============================================================================

#[test]
fn test_context_simple_position() {
    let source = "let x = 42";
    let ctx = CompletionContext::from_position(source, 0, 5);

    assert!(!ctx.after_dot);
    assert!(ctx.dot_target.is_none());
}

#[test]
fn test_context_after_dot() {
    let source = "Task.";
    let ctx = CompletionContext::from_position(source, 0, 5);

    assert!(ctx.after_dot);
    assert_eq!(ctx.dot_target, Some("Task".to_string()));
}

#[test]
fn test_context_after_dot_with_partial() {
    let source = "Task.ru";
    let ctx = CompletionContext::from_position(source, 0, 7);

    // After the partial "ru", we're not immediately after a dot
    assert!(!ctx.after_dot);
}

#[test]
fn test_context_variables_in_scope() {
    let source = "let x = 1\nlet y = 2\n";
    let ctx = CompletionContext::from_position(source, 2, 0);

    assert!(ctx.variables.contains(&"x".to_string()));
    assert!(ctx.variables.contains(&"y".to_string()));
}

#[test]
fn test_context_const_variables() {
    let source = "const PI = 3.14\nlet r = 5";
    let ctx = CompletionContext::from_position(source, 1, 10);

    assert!(ctx.variables.contains(&"PI".to_string()));
    assert!(ctx.variables.contains(&"r".to_string()));
}

#[test]
fn test_context_only_variables_before_cursor() {
    let source = "let x = 1\nlet y = 2\nlet z = 3";
    // Position at line 1, so only x and y should be in scope
    let ctx = CompletionContext::from_position(source, 1, 0);

    assert!(ctx.variables.contains(&"x".to_string()));
    assert!(ctx.variables.contains(&"y".to_string()));
    assert!(!ctx.variables.contains(&"z".to_string()));
}

#[test]
fn test_context_nested_module_access() {
    let source = "Promise.";
    let ctx = CompletionContext::from_position(source, 0, 8);

    assert!(ctx.after_dot);
    assert_eq!(ctx.dot_target, Some("Promise".to_string()));
}

// =============================================================================
// get_completions - top level tests
// =============================================================================

#[test]
fn test_completions_includes_keywords() {
    let source = "";
    let ctx = CompletionContext::from_position(source, 0, 0);
    let items = get_completions(&ctx);

    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    assert!(labels.contains(&"let"));
    assert!(labels.contains(&"const"));
    assert!(labels.contains(&"if"));
    assert!(labels.contains(&"else"));
    assert!(labels.contains(&"while"));
    assert!(labels.contains(&"for"));
    assert!(labels.contains(&"return"));
    assert!(labels.contains(&"await"));
    assert!(labels.contains(&"try"));
    assert!(labels.contains(&"catch"));
    assert!(labels.contains(&"break"));
    assert!(labels.contains(&"continue"));
    assert!(labels.contains(&"true"));
    assert!(labels.contains(&"false"));
    assert!(labels.contains(&"null"));
}

#[test]
fn test_completions_includes_modules() {
    let source = "";
    let ctx = CompletionContext::from_position(source, 0, 0);
    let items = get_completions(&ctx);

    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    assert!(labels.contains(&"Task"));
    assert!(labels.contains(&"Timer"));
    assert!(labels.contains(&"Signal"));
    assert!(labels.contains(&"Workflow"));
    assert!(labels.contains(&"Promise"));
    assert!(labels.contains(&"Math"));
    assert!(labels.contains(&"Inputs"));
}

#[test]
fn test_completions_includes_variables() {
    let source = "let myVar = 42\n";
    let ctx = CompletionContext::from_position(source, 1, 0);
    let items = get_completions(&ctx);

    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    assert!(labels.contains(&"myVar"));
}

#[test]
fn test_completions_correct_kinds() {
    let source = "let x = 1\n";
    let ctx = CompletionContext::from_position(source, 1, 0);
    let items = get_completions(&ctx);

    // Check keyword kind
    let let_item = items.iter().find(|i| i.label == "let").unwrap();
    assert_eq!(let_item.kind, Some(CompletionItemKind::KEYWORD));

    // Check module kind
    let task_item = items.iter().find(|i| i.label == "Task").unwrap();
    assert_eq!(task_item.kind, Some(CompletionItemKind::MODULE));

    // Check variable kind
    let x_item = items.iter().find(|i| i.label == "x").unwrap();
    assert_eq!(x_item.kind, Some(CompletionItemKind::VARIABLE));
}

// =============================================================================
// get_completions - module method tests
// =============================================================================

#[test]
fn test_completions_task_methods() {
    let source = "Task.";
    let ctx = CompletionContext::from_position(source, 0, 5);
    let items = get_completions(&ctx);

    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"run"));
    assert_eq!(items.len(), 1); // Task only has one method
}

#[test]
fn test_completions_timer_methods() {
    let source = "Timer.";
    let ctx = CompletionContext::from_position(source, 0, 6);
    let items = get_completions(&ctx);

    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"delay"));
}

#[test]
fn test_completions_signal_methods() {
    let source = "Signal.";
    let ctx = CompletionContext::from_position(source, 0, 7);
    let items = get_completions(&ctx);

    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"next"));
}

#[test]
fn test_completions_promise_methods() {
    let source = "Promise.";
    let ctx = CompletionContext::from_position(source, 0, 8);
    let items = get_completions(&ctx);

    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"all"));
    assert!(labels.contains(&"any"));
    assert!(labels.contains(&"race"));
}

#[test]
fn test_completions_math_methods() {
    let source = "Math.";
    let ctx = CompletionContext::from_position(source, 0, 5);
    let items = get_completions(&ctx);

    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"floor"));
    assert!(labels.contains(&"ceil"));
    assert!(labels.contains(&"abs"));
    assert!(labels.contains(&"round"));
    assert!(labels.contains(&"min"));
    assert!(labels.contains(&"max"));
}

#[test]
fn test_completions_method_kind() {
    let source = "Task.";
    let ctx = CompletionContext::from_position(source, 0, 5);
    let items = get_completions(&ctx);

    let run_item = items.iter().find(|i| i.label == "run").unwrap();
    assert_eq!(run_item.kind, Some(CompletionItemKind::METHOD));
}

#[test]
fn test_completions_method_has_signature() {
    let source = "Task.";
    let ctx = CompletionContext::from_position(source, 0, 5);
    let items = get_completions(&ctx);

    let run_item = items.iter().find(|i| i.label == "run").unwrap();
    assert!(run_item.detail.is_some());
    assert!(run_item.detail.as_ref().unwrap().contains("Task.run"));
}

// =============================================================================
// get_completions - array/string method tests
// =============================================================================

#[test]
fn test_completions_unknown_target_shows_array_string_methods() {
    let source = "myVar.";
    let ctx = CompletionContext::from_position(source, 0, 6);
    let items = get_completions(&ctx);

    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    // Should include array methods
    assert!(labels.contains(&"length"));
    assert!(labels.contains(&"concat"));
    assert!(labels.contains(&"includes"));
    assert!(labels.contains(&"indexOf"));
    assert!(labels.contains(&"join"));
    assert!(labels.contains(&"slice"));
    assert!(labels.contains(&"reverse"));

    // Should include string methods
    assert!(labels.contains(&"toUpperCase"));
    assert!(labels.contains(&"toLowerCase"));
    assert!(labels.contains(&"trim"));
    assert!(labels.contains(&"split"));
    assert!(labels.contains(&"startsWith"));
    assert!(labels.contains(&"endsWith"));
    assert!(labels.contains(&"replace"));
    assert!(labels.contains(&"substring"));
}

// =============================================================================
// get_signature_help tests
// =============================================================================

#[test]
fn test_signature_help_task_run() {
    let source = "Task.run(";
    let help = get_signature_help(source, 0, 9).expect("Should return signature help");

    assert_eq!(help.signatures.len(), 1);
    assert!(help.signatures[0].label.contains("Task.run"));
}

#[test]
fn test_signature_help_timer_delay() {
    let source = "Timer.delay(";
    let help = get_signature_help(source, 0, 12).expect("Should return signature help");

    assert_eq!(help.signatures.len(), 1);
    assert!(help.signatures[0].label.contains("Timer.delay"));
}

#[test]
fn test_signature_help_promise_all() {
    let source = "Promise.all(";
    let help = get_signature_help(source, 0, 12).expect("Should return signature help");

    assert_eq!(help.signatures.len(), 1);
    assert!(help.signatures[0].label.contains("Promise.all"));
}

#[test]
fn test_signature_help_inside_args() {
    let source = "Task.run(\"name\", ";
    let help = get_signature_help(source, 0, 17).expect("Should return signature help");

    assert_eq!(help.signatures.len(), 1);
    assert!(help.signatures[0].label.contains("Task.run"));
}

#[test]
fn test_signature_help_nested_parens() {
    let source = "Task.run(foo(";
    let help = get_signature_help(source, 0, 13);

    // This should not match Task.run since we're inside a nested call
    // The cursor is after the inner '(' so it won't find Task.run
    assert!(help.is_none());
}

#[test]
fn test_signature_help_no_match() {
    let source = "let x = 42";
    let help = get_signature_help(source, 0, 10);
    assert!(help.is_none());
}

#[test]
fn test_signature_help_unknown_function() {
    let source = "myFunc(";
    let help = get_signature_help(source, 0, 7);
    assert!(help.is_none());
}

// =============================================================================
// collect_variables tests
// =============================================================================

#[test]
fn test_collect_variables_simple() {
    let source = "let x = 1\nlet y = 2";
    let workflow = parse_workflow(source).expect("Should parse");
    let vars = collect_variables(&workflow.body);

    let names: Vec<&str> = vars.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"x"));
    assert!(names.contains(&"y"));
}

#[test]
fn test_collect_variables_const() {
    let source = "const PI = 3.14";
    let workflow = parse_workflow(source).expect("Should parse");
    let vars = collect_variables(&workflow.body);

    let names: Vec<&str> = vars.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"PI"));
}

#[test]
fn test_collect_variables_for_loop_binding() {
    let source = "for (let item of items) { }";
    let workflow = parse_workflow(source).expect("Should parse");
    let vars = collect_variables(&workflow.body);

    let names: Vec<&str> = vars.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"item"));
}

#[test]
fn test_collect_variables_try_catch() {
    let source = "try { } catch (err) { }";
    let workflow = parse_workflow(source).expect("Should parse");
    let vars = collect_variables(&workflow.body);

    let names: Vec<&str> = vars.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"err"));
}

#[test]
fn test_collect_variables_nested_in_if() {
    let source = "if (true) {\n  let inner = 1\n}";
    let workflow = parse_workflow(source).expect("Should parse");
    let vars = collect_variables(&workflow.body);

    let names: Vec<&str> = vars.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"inner"));
}

#[test]
fn test_collect_variables_nested_in_while() {
    let source = "while (true) {\n  let counter = 0\n}";
    let workflow = parse_workflow(source).expect("Should parse");
    let vars = collect_variables(&workflow.body);

    let names: Vec<&str> = vars.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"counter"));
}

#[test]
fn test_collect_variables_destructure() {
    let source = "let { a, b } = obj";
    let workflow = parse_workflow(source).expect("Should parse");
    let vars = collect_variables(&workflow.body);

    let names: Vec<&str> = vars.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"a"));
    assert!(names.contains(&"b"));
}

#[test]
fn test_collect_variables_has_correct_spans() {
    let source = "let x = 1";
    let workflow = parse_workflow(source).expect("Should parse");
    let vars = collect_variables(&workflow.body);

    assert_eq!(vars.len(), 1);
    let (name, span) = &vars[0];
    assert_eq!(name, "x");
    // 'x' starts at column 4 (0-indexed)
    assert_eq!(span.start_col, 4);
}

#[test]
fn test_collect_variables_if_else() {
    let source = "if (true) {\n  let a = 1\n} else {\n  let b = 2\n}";
    let workflow = parse_workflow(source).expect("Should parse");
    let vars = collect_variables(&workflow.body);

    let names: Vec<&str> = vars.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"a"));
    assert!(names.contains(&"b"));
}

#[test]
fn test_collect_variables_empty_workflow() {
    let source = "return null";
    let workflow = parse_workflow(source).expect("Should parse");
    let vars = collect_variables(&workflow.body);

    assert!(vars.is_empty());
}
