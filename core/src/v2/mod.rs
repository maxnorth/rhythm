//! V2 Interpreter - Next generation workflow execution engine
//!
//! This module contains the v2 implementation of the Rhythm workflow interpreter,
//! featuring a stack-based VM executor and modern parser.

pub mod client_adapter;
pub mod db;
pub mod executor;
pub mod parser;
pub mod types;
pub mod worker;

#[cfg(test)]
pub mod test_helpers;

#[cfg(test)]
mod tests;