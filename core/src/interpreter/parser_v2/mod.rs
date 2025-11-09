//! Parser v2 - PEST-based parser for Flow language
//!
//! Produces AST compatible with executor_v2

use pest::Parser;
use pest_derive::Parser;
use serde::{Deserialize, Serialize};

use super::executor_v2::types::ast::{Expr, Stmt};

pub mod semantic_validator;

#[cfg(test)]
mod tests;

/* ===================== Workflow Definition ===================== */

/// Workflow definition - represents a complete workflow file
///
/// A workflow file defines a single async function with parameters and a body.
/// Example:
/// ```js
/// async function workflow(input1, input2) {
///     let x = add(input1, input2)
///     return x
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDef {
    /// Parameter names (inputs to the workflow)
    pub params: Vec<String>,
    /// Workflow body (statements to execute)
    pub body: Stmt,
}

/* ===================== PEST Parser ===================== */

#[derive(Parser)]
#[grammar = "interpreter/parser_v2/flow_v2.pest"]
struct FlowParser;

/* ===================== Error Types ===================== */

#[derive(Debug)]
pub enum ParseError {
    PestError(String),
    BuildError(String),
}

impl From<pest::error::Error<Rule>> for ParseError {
    fn from(err: pest::error::Error<Rule>) -> Self {
        ParseError::PestError(err.to_string())
    }
}

pub type ParseResult<T> = Result<T, ParseError>;

/* ===================== Public API ===================== */

/// Parse a Flow source string into a workflow definition
///
/// ONLY accepts workflow function syntax: `async function workflow(params) { body }`
///
/// This is the production API. Use `parse()` for testing individual statements.
pub fn parse_workflow(source: &str) -> ParseResult<WorkflowDef> {
    let mut pairs = FlowParser::parse(Rule::program, source)?;

    let program = pairs.next().unwrap();

    // program = { SOI ~ (workflow_function | statement) ~ EOI }
    let content = program.into_inner().next().unwrap();

    match content.as_rule() {
        Rule::workflow_function => build_workflow_function(content),
        Rule::statement => {
            // Reject bare statements - must use workflow wrapper
            Err(ParseError::BuildError(
                "Workflow must be wrapped in 'async function workflow(...) { ... }'".to_string()
            ))
        }
        _ => Err(ParseError::BuildError(format!(
            "Unexpected program content: {:?}",
            content.as_rule()
        ))),
    }
}

/// Parse a Flow source string into an AST statement (testing API)
///
/// This function allows parsing bare statements for testing parser internals.
/// It bypasses the workflow wrapper requirement.
///
/// Production code should use `parse_workflow` which enforces the wrapper.
pub fn parse(source: &str) -> ParseResult<Stmt> {
    let mut pairs = FlowParser::parse(Rule::program, source)?;
    let program = pairs.next().unwrap();
    let content = program.into_inner().next().unwrap();

    match content.as_rule() {
        Rule::workflow_function => {
            // If it's a workflow function, extract the body
            let workflow = build_workflow_function(content)?;
            Ok(workflow.body)
        }
        Rule::statement => {
            // Allow bare statements for testing
            build_statement(content)
        }
        _ => Err(ParseError::BuildError(format!(
            "Unexpected program content: {:?}",
            content.as_rule()
        ))),
    }
}

/* ===================== AST Builder ===================== */

fn build_workflow_function(pair: pest::iterators::Pair<Rule>) -> ParseResult<WorkflowDef> {
    // workflow_function = { "async" ~ "function" ~ identifier ~ "(" ~ param_list? ~ ")" ~ block }
    let mut inner = pair.into_inner();

    // Skip "async" and "function" keywords (they're literal matches, not captured)
    // Get function name (we ignore it for now, but it's required by syntax)
    let _name = inner.next().unwrap(); // identifier

    // Get parameters
    let next = inner.next().unwrap();
    let (params, block_pair) = if next.as_rule() == Rule::param_list {
        // Has parameters
        let params = build_param_list(next)?;
        let block = inner.next().unwrap();
        (params, block)
    } else {
        // No parameters, next is the block
        (vec![], next)
    };

    // Build body from block
    let body = build_block(block_pair)?;

    Ok(WorkflowDef { params, body })
}

fn build_param_list(pair: pest::iterators::Pair<Rule>) -> ParseResult<Vec<String>> {
    // param_list = { identifier ~ ("," ~ identifier)* }
    let params = pair
        .into_inner()
        .map(|id_pair| id_pair.as_str().to_string())
        .collect();
    Ok(params)
}

fn build_block(pair: pest::iterators::Pair<Rule>) -> ParseResult<Stmt> {
    // block = { "{" ~ statement* ~ "}" }
    let statements: Result<Vec<Stmt>, ParseError> = pair
        .into_inner()
        .map(|stmt_pair| build_statement(stmt_pair))
        .collect();

    Ok(Stmt::Block {
        body: statements?,
    })
}

fn build_statement(pair: pest::iterators::Pair<Rule>) -> ParseResult<Stmt> {
    match pair.as_rule() {
        Rule::statement => {
            // statement = { return_stmt }
            let inner = pair.into_inner().next().unwrap();
            build_statement(inner)
        }
        Rule::return_stmt => {
            // return_stmt = { "return" ~ expression }
            let mut inner = pair.into_inner();
            let expr_pair = inner.next().unwrap();
            let expr = build_expression(expr_pair)?;
            Ok(Stmt::Return { value: Some(expr) })
        }
        _ => Err(ParseError::BuildError(format!(
            "Unexpected statement rule: {:?}",
            pair.as_rule()
        ))),
    }
}

fn build_expression(pair: pest::iterators::Pair<Rule>) -> ParseResult<Expr> {
    match pair.as_rule() {
        Rule::expression => {
            // expression = { member_expr }
            let inner = pair.into_inner().next().unwrap();
            build_expression(inner)
        }
        Rule::member_expr => {
            // member_expr = { primary ~ ("." ~ identifier)* }
            let mut inner = pair.into_inner();

            // Start with the primary expression
            let primary = inner.next().unwrap();
            let mut expr = build_expression(primary)?;

            // Chain member accesses left-to-right
            for property_pair in inner {
                let property = property_pair.as_str().to_string();
                expr = Expr::Member {
                    object: Box::new(expr),
                    property,
                };
            }

            Ok(expr)
        }
        Rule::primary => {
            // primary = { literal | identifier }
            let inner = pair.into_inner().next().unwrap();
            build_expression(inner)
        }
        Rule::identifier => {
            let name = pair.as_str().to_string();
            Ok(Expr::Ident { name })
        }
        Rule::literal => {
            // literal = { boolean | number | string | null_lit }
            let inner = pair.into_inner().next().unwrap();
            build_expression(inner)
        }
        Rule::number => {
            let num_str = pair.as_str();
            let value = num_str.parse::<f64>().map_err(|e| {
                ParseError::BuildError(format!("Failed to parse number '{}': {}", num_str, e))
            })?;
            Ok(Expr::LitNum { v: value })
        }
        Rule::boolean => {
            let value = pair.as_str() == "true";
            Ok(Expr::LitBool { v: value })
        }
        Rule::string => {
            // string = { "\"" ~ string_content ~ "\"" }
            let mut inner = pair.into_inner();
            let content = inner.next().unwrap();
            let value = content.as_str().to_string();
            Ok(Expr::LitStr { v: value })
        }
        Rule::null_lit => Ok(Expr::LitNull),
        _ => Err(ParseError::BuildError(format!(
            "Unexpected expression rule: {:?}",
            pair.as_rule()
        ))),
    }
}
