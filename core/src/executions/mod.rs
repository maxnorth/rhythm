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
pub use claim::claim_execution;
pub use create::create_execution;
pub use lifecycle::{cancel_execution, complete_execution, fail_execution};
pub use query::{get_execution, get_workflow_tasks, list_executions};
