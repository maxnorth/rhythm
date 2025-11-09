//! Parser v2 - PEST-based parser for Flow language
//!
//! Produces AST compatible with executor_v2

use pest::Parser;
use pest_derive::Parser;

use super::executor_v2::types::ast::{Expr, Stmt};

#[cfg(test)]
mod tests;

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

/// Parse a Flow source string into an AST statement
pub fn parse(source: &str) -> ParseResult<Stmt> {
    let mut pairs = FlowParser::parse(Rule::program, source)?;

    let program = pairs.next().unwrap();

    // program = { SOI ~ statement ~ EOI }
    let statement = program.into_inner().next().unwrap();

    build_statement(statement)
}

/* ===================== AST Builder ===================== */

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
            // expression = { literal }
            let inner = pair.into_inner().next().unwrap();
            build_expression(inner)
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
        Rule::null_lit => Ok(Expr::LitBool { v: false }), // Temporary: map null to false
        _ => Err(ParseError::BuildError(format!(
            "Unexpected expression rule: {:?}",
            pair.as_rule()
        ))),
    }
}
