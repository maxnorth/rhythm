//! Tests for the semantic validation system

use super::*;
use crate::parser::parse_workflow;

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse source and validate, returning errors
fn validate(source: &str) -> Vec<ValidationError> {
    let workflow = parse_workflow(source).expect("Parse should succeed");
    validate_workflow(&workflow, source)
}

/// Check if errors contain a specific rule
fn has_rule(errors: &[ValidationError], rule_id: &str) -> bool {
    errors.iter().any(|e| e.rule_id == rule_id)
}

/// Get errors for a specific rule
fn for_rule<'a>(errors: &'a [ValidationError], rule_id: &str) -> Vec<&'a ValidationError> {
    errors.iter().filter(|e| e.rule_id == rule_id).collect()
}

// ============================================================================
// Undefined Variable Tests
// ============================================================================

#[test]
fn test_undefined_variable_simple() {
    let source = "let y = x + 1";

    let errors = validate(source);
    assert!(has_rule(&errors, "undefined-variable"));

    let undef_errors = for_rule(&errors, "undefined-variable");
    assert_eq!(undef_errors.len(), 1);
    assert!(undef_errors[0].message.contains("'x'"));
}

#[test]
fn test_undefined_variable_ok_when_declared() {
    let source = r#"
let x = 5
let y = x + 1
"#;

    let errors = validate(source);
    assert!(!has_rule(&errors, "undefined-variable"));
}

#[test]
fn test_undefined_variable_builtins_ok() {
    let source = r#"
let result = await task.run("my-task", {})
await timer.delay(1000)
"#;

    let errors = validate(source);
    let undef_errors = for_rule(&errors, "undefined-variable");
    assert!(
        undef_errors.is_empty(),
        "Built-ins should not be flagged as undefined"
    );
}

#[test]
fn test_undefined_variable_self_reference() {
    let source = "let x = x + 1";

    let errors = validate(source);
    assert!(has_rule(&errors, "undefined-variable"));
}

#[test]
fn test_undefined_variable_for_loop_binding() {
    let source = r#"
let items = [1, 2, 3]
for (let item of items) {
    let doubled = item * 2
}
"#;

    let errors = validate(source);
    let undef_errors = for_rule(&errors, "undefined-variable");
    assert!(
        undef_errors.is_empty(),
        "For loop binding should be in scope"
    );
}

#[test]
fn test_undefined_variable_try_catch() {
    let source = r#"
try {
    let x = 1
} catch (err) {
    let msg = err.message
}
"#;

    let errors = validate(source);
    let undef_errors = for_rule(&errors, "undefined-variable");
    assert!(undef_errors.is_empty(), "Catch variable should be in scope");
}

// ============================================================================
// Unused Variable Tests
// ============================================================================

#[test]
fn test_unused_variable_simple() {
    let source = r#"
let x = 5
return 10
"#;

    let errors = validate(source);
    assert!(has_rule(&errors, "unused-variable"));

    let unused_errors = for_rule(&errors, "unused-variable");
    assert_eq!(unused_errors.len(), 1);
    assert!(unused_errors[0].message.contains("'x'"));
}

#[test]
fn test_unused_variable_ok_when_used() {
    let source = r#"
let x = 5
return x
"#;

    let errors = validate(source);
    assert!(!has_rule(&errors, "unused-variable"));
}

#[test]
fn test_unused_variable_underscore_prefix() {
    let source = r#"
let _unused = 5
return 10
"#;

    let errors = validate(source);
    let unused_errors = for_rule(&errors, "unused-variable");
    assert!(
        unused_errors.is_empty(),
        "Underscore-prefixed variables should be exempt"
    );
}

#[test]
fn test_unused_variable_destructure() {
    let source = r#"
let {a, b} = {a: 1, b: 2}
return a
"#;

    let errors = validate(source);
    let unused_errors = for_rule(&errors, "unused-variable");
    assert!(unused_errors.iter().any(|e| e.message.contains("'b'")));
}

// ============================================================================
// Unreachable Code Tests
// ============================================================================

#[test]
fn test_unreachable_after_return() {
    let source = r#"
return 5
let x = 10
"#;

    let errors = validate(source);
    assert!(has_rule(&errors, "unreachable-code"));
}

#[test]
fn test_unreachable_ok_after_conditional_return() {
    let source = r#"
if (true) {
    return 5
}
let x = 10
return x
"#;

    let errors = validate(source);
    let unreachable = for_rule(&errors, "unreachable-code");
    assert!(
        unreachable.is_empty(),
        "Code after single-branch return should be reachable"
    );
}

#[test]
fn test_unreachable_after_both_branches_return() {
    let source = r#"
if (true) {
    return 5
} else {
    return 10
}
let x = 10
"#;

    let errors = validate(source);
    assert!(has_rule(&errors, "unreachable-code"));
}

#[test]
fn test_unreachable_nested_block() {
    let source = r#"
{
    return 5
    let x = 10
}
"#;

    let errors = validate(source);
    assert!(has_rule(&errors, "unreachable-code"));
}

// ============================================================================
// Validator Integration Tests
// ============================================================================

#[test]
fn test_validator_runs_all_rules() {
    let validator = Validator::new();
    let rules: Vec<_> = validator.rules().collect();

    assert!(rules.len() >= 3);
    assert!(rules.iter().any(|(id, _)| *id == "undefined-variable"));
    assert!(rules.iter().any(|(id, _)| *id == "unused-variable"));
    assert!(rules.iter().any(|(id, _)| *id == "unreachable-code"));
}

#[test]
fn test_validator_multiple_errors() {
    let source = r#"
let unused = 5
let y = undefined_var
return 1
let unreachable = 2
"#;

    let errors = validate(source);

    assert!(has_rule(&errors, "unused-variable"));
    assert!(has_rule(&errors, "undefined-variable"));
    assert!(has_rule(&errors, "unreachable-code"));
}

#[test]
fn test_has_errors_function() {
    let workflow = parse_workflow("let y = x").expect("Should parse");
    assert!(has_errors(&workflow, "let y = x"));

    let workflow = parse_workflow("let x = 5\nreturn x").expect("Should parse");
    assert!(!has_errors(&workflow, "let x = 5\nreturn x"));
}

#[test]
fn test_error_display() {
    let source = "let y = x";
    let errors = validate(source);

    assert!(!errors.is_empty());
    let display = format!("{}", errors[0]);
    assert!(display.contains("undefined-variable"));
    assert!(display.contains("'x'"));
}

#[test]
fn test_clean_code_no_errors() {
    let source = r#"
let x = 5
let y = x + 1
return y
"#;

    let errors = validate(source);
    assert!(errors.is_empty(), "Clean code should have no errors");
}
