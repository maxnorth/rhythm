//! # Executor V2 - Resumable Stack-Driven Interpreter
//!
//! Clean rewrite following the design from `.context/executor/docs.md`.
//!
//! ## Core Principles
//!
//! 1. **Stack-driven execution**: All state in `frames: Vec<Frame>`, no recursion
//! 2. **Statement-level execution**: Each frame has a PC tracking micro-steps
//! 3. **Centralized control flow**: `Control` enum manages break/continue/return/throw
//! 4. **Pure executor**: No DB, no async - just runs until suspend or complete
//!
//! ## Implementation Milestones
//!
//! - [x] **Milestone 1**: Core execution loop with Return statement only
//! - [ ] **Milestone 2**: Let and Assign statements (variables)
//! - [ ] **Milestone 3**: Block statement with proper scoping
//! - [ ] **Milestone 4**: If/While control flow
//! - [ ] **Milestone 5**: For loops
//! - [ ] **Milestone 6**: Try/Catch/Finally with unwinding
//! - [ ] **Milestone 7**: Await/suspend/resume
//! - [ ] **Milestone 8**: Task outbox and stdlib integration

pub mod exec_loop;
pub mod expressions;
pub mod statements;
pub mod types;
pub mod vm;

#[cfg(test)]
mod tests;

// Re-export commonly used items
pub use exec_loop::{run_until_done, step};
pub use expressions::EvalResult;
pub use types::{Control, Expr, Stmt, Val};
pub use vm::{Step, VM};
