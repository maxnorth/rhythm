//! Test helpers for executor_v2 tests
//!
//! Common utilities for parsing workflows and building VMs

use crate::interpreter::executor_v2::{Val, VM};
use crate::interpreter::parser_v2::{self, WorkflowDef};
use std::collections::HashMap;

/// Parse workflow source, validate, serialize/deserialize, and create VM
///
/// This helper:
/// - Parses the workflow using parser_v2
/// - Validates the workflow semantically
/// - Serializes and deserializes (to test round-trip compatibility)
/// - Creates a VM with proper environment setup
///
/// # Arguments
/// * `source` - Workflow source code (must use `async function workflow(...)` syntax)
/// * `inputs` - Input values to pass as the second parameter to the workflow
///
/// # Returns
/// A VM ready to execute with `run_until_done()` or `step()`
///
/// # Environment Setup
/// - If workflow has 1+ params: First param (`ctx`) = empty object
/// - If workflow has 2+ params: Second param (`inputs`) = provided inputs map
pub fn parse_workflow_and_build_vm(source: &str, inputs: HashMap<String, Val>) -> VM {
    let workflow = parser_v2::parse_workflow(source).expect("Parse workflow failed");
    parser_v2::semantic_validator::validate_workflow(&workflow)
        .expect("Workflow validation failed");
    let json = serde_json::to_string(&workflow).expect("Workflow serialization failed");
    let workflow: WorkflowDef =
        serde_json::from_str(&json).expect("Workflow deserialization failed");

    let mut env = HashMap::new();
    if workflow.params.len() >= 1 {
        env.insert(workflow.params[0].clone(), Val::Obj(HashMap::new()));
    }
    if workflow.params.len() >= 2 {
        env.insert(workflow.params[1].clone(), Val::Obj(inputs));
    }

    VM::new(workflow.body.clone(), env)
}
