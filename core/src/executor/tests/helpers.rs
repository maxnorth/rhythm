//! Test helpers for executor_v2 tests
//!
//! Common utilities for parsing workflows and building VMs

use crate::executor::{Val, WorkflowContext, VM};
use crate::parser::WorkflowDef;
use std::collections::HashMap;

/// Parse workflow source, validate, serialize/deserialize, and create VM
///
/// This helper:
/// - Parses the workflow using parser_v2
/// - Validates the workflow semantically
/// - Serializes and deserializes (to test round-trip compatibility)
/// - Creates a VM with Context, Inputs, and stdlib injected
///
/// # Arguments
/// * `source` - Workflow source code
/// * `inputs` - Input values to pass as Inputs
///
/// # Returns
/// A VM ready to execute with `run_until_done()` or `step()`
pub fn parse_workflow_and_build_vm(source: &str, inputs: HashMap<String, Val>) -> VM {
    let workflow = crate::parser::parse_workflow(source).expect("Parse workflow failed");
    let errors = crate::parser::semantic_validator::validate_workflow(&workflow, source);
    let validation_errors: Vec<_> = errors.iter().filter(|e| e.is_error()).collect();
    assert!(
        validation_errors.is_empty(),
        "Workflow validation failed: {:?}",
        validation_errors
    );
    let json = serde_json::to_string(&workflow).expect("Workflow serialization failed");
    let workflow: WorkflowDef =
        serde_json::from_str(&json).expect("Workflow deserialization failed");

    let context = WorkflowContext {
        execution_id: "test-execution-id".to_string(),
    };
    VM::new(workflow.body.clone(), inputs, context)
}

/// Parse workflow source WITHOUT validation, for testing runtime error behavior.
///
/// Use this helper when testing that the runtime correctly handles errors
/// that would now be caught by semantic validation (e.g., undefined variables,
/// out-of-scope access). These tests verify runtime robustness.
///
/// # Arguments
/// * `source` - Workflow source code (may contain semantic errors)
/// * `inputs` - Input values to pass as Inputs
///
/// # Returns
/// A VM ready to execute with `run_until_done()` or `step()`
pub fn parse_workflow_without_validation(source: &str, inputs: HashMap<String, Val>) -> VM {
    let workflow = crate::parser::parse_workflow(source).expect("Parse workflow failed");
    let json = serde_json::to_string(&workflow).expect("Workflow serialization failed");
    let workflow: WorkflowDef =
        serde_json::from_str(&json).expect("Workflow deserialization failed");

    let context = WorkflowContext {
        execution_id: "test-execution-id".to_string(),
    };
    VM::new(workflow.body.clone(), inputs, context)
}
