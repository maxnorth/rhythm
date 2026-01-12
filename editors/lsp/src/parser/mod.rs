//! Parser for Rhythm language
//!
//! This module re-exports rhythm-core's parser and AST types for use by the LSP.

// Re-export core AST types
pub use rhythm_core::executor::types::ast::{DeclareTarget, Expr, Span, Stmt};
pub use rhythm_core::parser::WorkflowDef;

/// Parse error with location information
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Option<Span>,
}

impl ParseError {
    #[allow(dead_code)]
    pub fn new(message: String, span: Option<Span>) -> Self {
        Self { message, span }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

pub type ParseResult<T> = Result<T, ParseError>;

/// Parse a Rhythm source string into a workflow definition
pub fn parse_workflow(source: &str) -> ParseResult<WorkflowDef> {
    match rhythm_core::parser::parse_workflow(source) {
        Ok(workflow) => Ok(workflow),
        Err(e) => {
            let span = e.span();
            Err(ParseError {
                message: e.to_string(),
                span,
            })
        }
    }
}

#[cfg(test)]
mod tests;
