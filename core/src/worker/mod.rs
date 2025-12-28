//! V2 Worker
//!
//! This module provides the worker loop logic for claiming and executing work.

pub mod awaitable;
pub mod claim;
pub mod complete;
pub mod runner;
pub mod signals;

#[cfg(test)]
mod tests;

// Re-export public API
pub use claim::{run_cooperative_worker_loop, DelegatedAction};
pub use complete::complete_work;
pub use runner::run_workflow;
