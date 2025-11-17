//! Error codes and utilities
//!
//! Defines standard error codes used throughout the executor

use serde::{Deserialize, Serialize};

/// Error information with code and message
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorInfo {
    /// Error code (e.g., "PROPERTY_NOT_FOUND", "TYPE_ERROR")
    pub code: String,
    /// Human-readable error message
    pub message: String,
}

impl ErrorInfo {
    /// Create a new error with code and message
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Error code: Property not found on object
pub const PROPERTY_NOT_FOUND: &str = "PROPERTY_NOT_FOUND";

/// Error code: Type error (wrong type for operation)
pub const TYPE_ERROR: &str = "TYPE_ERROR";

/// Error code: Undefined variable
pub const UNDEFINED_VARIABLE: &str = "UNDEFINED_VARIABLE";

/// Error code: Internal error (should not happen - validator bug)
pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";

/// Error code: Value is not callable
pub const NOT_A_FUNCTION: &str = "NOT_A_FUNCTION";

/// Error code: Wrong number of arguments
pub const WRONG_ARG_COUNT: &str = "WRONG_ARG_COUNT";

/// Error code: Wrong argument type
pub const WRONG_ARG_TYPE: &str = "WRONG_ARG_TYPE";
