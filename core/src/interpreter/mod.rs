//! DSL workflow interpreter
//!
//! This module provides parsing and execution for DSL-based workflows.
//!
//! ## Architecture
//!
//! - **Parser** (`parser.rs`): Parses `.flow` files into JSON AST
//! - **Executor** (`executor.rs`): Tree-walking interpreter that executes workflows step-by-step
//!
//! ## Usage
//!
//! Workflows are registered via language adapters (Python, Node.js, etc.) using `workflows::register_workflows()`.
//! The actual registration logic lives in `workflows.rs`, not here.
//!
//! This module only provides the core parsing and execution primitives.

pub mod parser;
pub mod executor;

pub use parser::{parse_workflow, ParseError};
pub use executor::{execute_workflow_step, StepResult};
