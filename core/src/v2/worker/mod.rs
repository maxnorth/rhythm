//! V2 Worker
//!
//! This module provides the worker loop logic for claiming and executing work.

pub mod claim;
pub mod complete;
pub mod runner;

// Re-export public API
pub use claim::{claim_work, ClaimedTask};
pub use complete::complete_work;
pub use runner::run_workflow;
