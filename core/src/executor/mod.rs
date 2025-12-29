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
//! - [x] **Milestone 2**: Assign statements (variables and attribute assignment)
//! - [x] **Milestone 3**: Block statement with proper scoping
//! - [x] **Milestone 4**: If control flow
//! - [x] **Milestone 5**: While loops with Break/Continue
//! - [ ] **Milestone 6**: For loops
//! - [x] **Milestone 7**: Try/Catch with unwinding
//! - [x] **Milestone 8**: Await/suspend/resume
//! - [x] **Milestone 9**: Task outbox and stdlib integration
//! - [x] **Milestone 10**: Expression evaluation with literals, member access, calls
//!
//! ## Completed Features
//!
//! - Core execution loop with frame-based stack
//! - Return, Block, Try/Catch, Expr, Assign, If, While, Break, Continue statements
//! - Expression evaluation (literals, identifiers, member access, function calls, await)
//! - Attribute assignment with runtime type checking (obj.prop = value, arr[i] = value)
//! - Control flow: Return, Throw, Break, Continue
//! - Suspend/Resume for async task execution
//! - Standard library (Math, Task modules, arithmetic and comparison operators)

pub mod errors;
pub mod exec_loop;
pub mod expressions;
pub mod json;
pub mod outbox;
pub mod statements;
pub mod stdlib;
pub mod types;
pub mod vm;

#[cfg(test)]
mod tests;

// Re-export commonly used items
pub use exec_loop::{run_until_done, step};
pub use expressions::EvalResult;
pub use json::{json_to_val, json_to_val_map, val_map_to_json, val_to_json};
pub use outbox::{Outbox, TaskCreation, TimerSchedule};
pub use types::{Awaitable, Control, ErrorInfo, Expr, Stmt, Val};
pub use vm::VM;
