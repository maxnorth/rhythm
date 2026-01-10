use crate::parser::parse_workflow;

#[test]
fn test_parse_simple() {
    let source = "let x = 42\nreturn x";
    let result = parse_workflow(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_workflow_with_builtins() {
    let source = r#"
let orderId = Inputs.orderId
let result = await Task.run("process", { id: orderId })
return result
"#;

    let result = parse_workflow(source);
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
}

#[test]
fn test_parse_error_location() {
    let source = "let x = @invalid";
    let result = parse_workflow(source);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.span.is_some());
}

#[test]
fn test_span_tracking() {
    let source = "let x = 42";
    let result = parse_workflow(source).unwrap();
    assert_eq!(result.span.start, 0);
    assert_eq!(result.span.end, 10);
}
