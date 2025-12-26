//! Test helpers for executor_v2 tests
//!
//! Common utilities for parsing workflows and building VMs

use crate::executor::{Val, VM};
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
    crate::parser::semantic_validator::validate_workflow(&workflow)
        .expect("Workflow validation failed");
    let json = serde_json::to_string(&workflow).expect("Workflow serialization failed");
    let workflow: WorkflowDef =
        serde_json::from_str(&json).expect("Workflow deserialization failed");

    VM::new(workflow.body.clone(), inputs)
}
