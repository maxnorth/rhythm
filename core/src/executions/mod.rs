// Execution management module
//
// This module handles all execution-related operations:
// - Creating executions (with idempotency support)
// - Claiming executions for workers
// - Managing execution lifecycle (complete, fail, suspend, resume)
// - Querying executions

mod claim;
mod create;
mod lifecycle;
mod query;

#[cfg(test)]
mod tests;

// Re-export public API
pub use claim::{claim_execution, claim_executions_batch};
pub use create::create_execution;
pub use lifecycle::{
    cancel_execution, complete_execution, complete_executions_batch, fail_execution,
    resume_workflow, suspend_workflow,
};
pub use query::{get_execution, get_workflow_tasks, list_executions};
