//! Tests for the validation system

use super::*;
use crate::parser::parse_workflow;

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse source and validate, returning diagnostics
fn validate(source: &str) -> Vec<Diagnostic> {
    let workflow = parse_workflow(source).expect("Parse should succeed");
    let validator = Validator::new();
    validator.validate(&workflow, source)
}

/// Check if diagnostics contain a specific rule
fn has_rule(diagnostics: &[Diagnostic], rule_id: &str) -> bool {
    diagnostics.iter().any(|d| d.rule_id == rule_id)
}

/// Get diagnostics for a specific rule
fn for_rule<'a>(diagnostics: &'a [Diagnostic], rule_id: &str) -> Vec<&'a Diagnostic> {
    diagnostics
        .iter()
        .filter(|d| d.rule_id == rule_id)
        .collect()
}

// ============================================================================
// Undefined Variable Tests
// ============================================================================

#[test]
fn test_undefined_variable_simple() {
    let source = "let y = x + 1";

    let diagnostics = validate(source);
    assert!(has_rule(&diagnostics, "undefined-variable"));

    let errors = for_rule(&diagnostics, "undefined-variable");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("'x'"));
}

#[test]
fn test_undefined_variable_ok_when_declared() {
    let source = r#"
let x = 5
let y = x + 1
"#;

    let diagnostics = validate(source);
    assert!(!has_rule(&diagnostics, "undefined-variable"));
}

#[test]
fn test_undefined_variable_builtins_ok() {
    let source = r#"
let result = await task.run("my-task", {})
await timer.delay(1000)
"#;

    let diagnostics = validate(source);
    // Should not report task, timer as undefined
    let errors = for_rule(&diagnostics, "undefined-variable");
    assert!(
        errors.is_empty(),
        "Built-ins should not be flagged as undefined"
    );
}

#[test]
fn test_undefined_variable_self_reference() {
    // let x = x + 1 should be an error
    let source = "let x = x + 1";

    let diagnostics = validate(source);
    assert!(has_rule(&diagnostics, "undefined-variable"));
}

#[test]
fn test_undefined_variable_for_loop_binding() {
    let source = r#"
let items = [1, 2, 3]
for (let item of items) {
    let doubled = item * 2
}
"#;

    let diagnostics = validate(source);
    // 'item' should be in scope within the loop
    let errors = for_rule(&diagnostics, "undefined-variable");
    assert!(errors.is_empty(), "For loop binding should be in scope");
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

    let diagnostics = validate(source);
    // 'err' should be in scope in the catch block
    let errors = for_rule(&diagnostics, "undefined-variable");
    assert!(errors.is_empty(), "Catch variable should be in scope");
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

    let diagnostics = validate(source);
    assert!(has_rule(&diagnostics, "unused-variable"));

    let warnings = for_rule(&diagnostics, "unused-variable");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].message.contains("'x'"));
}

#[test]
fn test_unused_variable_ok_when_used() {
    let source = r#"
let x = 5
return x
"#;

    let diagnostics = validate(source);
    assert!(!has_rule(&diagnostics, "unused-variable"));
}

#[test]
fn test_unused_variable_underscore_prefix() {
    let source = r#"
let _unused = 5
return 10
"#;

    let diagnostics = validate(source);
    // Variables starting with _ should not be flagged
    let warnings = for_rule(&diagnostics, "unused-variable");
    assert!(
        warnings.is_empty(),
        "Underscore-prefixed variables should be exempt"
    );
}

#[test]
fn test_unused_variable_destructure() {
    let source = r#"
let {a, b} = {a: 1, b: 2}
return a
"#;

    let diagnostics = validate(source);
    // 'b' is unused
    let warnings = for_rule(&diagnostics, "unused-variable");
    assert!(warnings.iter().any(|w| w.message.contains("'b'")));
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

    let diagnostics = validate(source);
    assert!(has_rule(&diagnostics, "unreachable-code"));
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

    let diagnostics = validate(source);
    // Code after if is reachable because not both branches return
    let warnings = for_rule(&diagnostics, "unreachable-code");
    assert!(
        warnings.is_empty(),
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

    let diagnostics = validate(source);
    assert!(has_rule(&diagnostics, "unreachable-code"));
}

#[test]
fn test_unreachable_nested_block() {
    let source = r#"
{
    return 5
    let x = 10
}
"#;

    let diagnostics = validate(source);
    assert!(has_rule(&diagnostics, "unreachable-code"));
}

// ============================================================================
// Validator Integration Tests
// ============================================================================

#[test]
fn test_validator_runs_all_rules() {
    let validator = Validator::new();
    let rules: Vec<_> = validator.rules().collect();

    // Should have at least our 3 rules
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

    let diagnostics = validate(source);

    // Should find all three issues
    assert!(has_rule(&diagnostics, "unused-variable"));
    assert!(has_rule(&diagnostics, "undefined-variable"));
    assert!(has_rule(&diagnostics, "unreachable-code"));
}

#[test]
fn test_diagnostic_to_lsp() {
    let source = "let y = x";

    let diagnostics = validate(source);
    assert!(!diagnostics.is_empty());

    // Convert to LSP diagnostic
    let lsp_diag = diagnostics[0].to_lsp_diagnostic();
    assert_eq!(lsp_diag.source, Some("rhythm".to_string()));
    assert!(lsp_diag.message.contains('x'));
}

#[test]
fn test_clean_code_no_diagnostics() {
    let source = r#"
let x = 5
let y = x + 1
return y
"#;

    let diagnostics = validate(source);
    assert!(
        diagnostics.is_empty(),
        "Clean code should have no diagnostics"
    );
}
