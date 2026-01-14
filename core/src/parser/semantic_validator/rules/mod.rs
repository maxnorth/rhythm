//! Validation Rules
//!
//! Each file in this module contains one validation rule.
//! Rules are organized by what they check:
//!
//! - `undefined_variable.rs` - Variables used before declaration
//! - `unused_variable.rs` - Variables declared but never used
//! - `unreachable_code.rs` - Code that can never execute
//! - `nested_await.rs` - Await expressions nested inside other expressions

mod nested_await;
mod undefined_variable;
mod unreachable_code;
mod unused_variable;

pub use nested_await::NestedAwaitRule;
pub use undefined_variable::UndefinedVariableRule;
pub use unreachable_code::UnreachableCodeRule;
pub use unused_variable::UnusedVariableRule;
