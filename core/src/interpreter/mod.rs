pub mod parser;
pub mod executor;

pub use parser::{parse_workflow, ParseError};
pub use executor::{execute_workflow_step, StepResult};

use std::path::PathBuf;
use serde_json::Value as JsonValue;

/// Configuration for workflow initialization
#[derive(Debug, Clone)]
pub struct WorkflowConfig {
    /// Paths to directories containing .crnt workflow files
    pub workflow_paths: Vec<PathBuf>,
}

impl WorkflowConfig {
    pub fn new() -> Self {
        Self {
            workflow_paths: Vec::new(),
        }
    }

    pub fn add_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.workflow_paths.extend(paths);
        self
    }
}

/// Initialize workflows from the configured paths
///
/// This function:
/// 1. Scans all workflow_paths for .crnt files
/// 2. Parses each file to JSON steps
/// 3. Hashes file content to generate version
/// 4. Stores workflow definitions in database
///
/// # Example
/// ```no_run
/// use currant_core::interpreter::{WorkflowConfig, initialize_workflows};
///
/// let config = WorkflowConfig::new()
///     .add_path("./workflows")
///     .add_path("./app/workflows");
///
/// initialize_workflows(config).await?;
/// ```
pub async fn initialize_workflows(config: WorkflowConfig) -> Result<(), WorkflowError> {
    // TODO: 1. Scan all workflow_paths for .crnt files
    //       - Use walkdir or similar to recursively find files
    //       - Filter by .crnt extension
    //       - Collect (path, filename) pairs

    // TODO: 2. For each .crnt file:
    //       - Read file contents as string
    //       - Extract workflow name from filename (strip .crnt extension)
    //       - Hash file contents to generate version_hash (SHA256)

    // TODO: 3. Parse workflow to JSON steps
    //       - Call parser::parse_workflow(source)
    //       - Serialize Vec<JsonValue> to JSONB for storage
    //       - Handle parse errors gracefully (log and continue? or fail?)

    // TODO: 4. Store in database (workflow_definitions table)
    //       - INSERT INTO workflow_definitions (name, version_hash, dsl_text, json_steps)
    //       - ON CONFLICT (name, version_hash) DO NOTHING
    //       - This makes initialization idempotent

    // TODO: 5. Return success or error
    //       - If any critical failures, return Err
    //       - Maybe collect all errors and return summary?

    Ok(())
}

/// Represents a parsed workflow definition ready for storage
#[derive(Debug, Clone)]
pub struct WorkflowDefinition {
    pub name: String,
    pub version_hash: String,
    pub dsl_text: String,
    pub json_steps: Vec<JsonValue>,
}

#[derive(Debug)]
pub enum WorkflowError {
    ParseError { file: PathBuf, error: ParseError },
    IoError { file: PathBuf, error: std::io::Error },
    DatabaseError(String),
}

impl std::fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowError::ParseError { file, error } => {
                write!(f, "Failed to parse workflow {}: {}", file.display(), error)
            }
            WorkflowError::IoError { file, error } => {
                write!(f, "Failed to read workflow {}: {}", file.display(), error)
            }
            WorkflowError::DatabaseError(msg) => {
                write!(f, "Database error: {}", msg)
            }
        }
    }
}

impl std::error::Error for WorkflowError {}
