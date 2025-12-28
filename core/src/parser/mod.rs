//! Parser v2 - PEST-based parser for Flow language
//!
//! Produces AST compatible with executor_v2

use pest::Parser;
use pest_derive::Parser;
use serde::{Deserialize, Serialize};

use super::executor::types::ast::{
    BinaryOp, DeclareTarget, Expr, ForLoopKind, MemberAccess, Stmt, VarKind,
};

pub mod semantic_validator;

#[cfg(test)]
mod tests;

/* ===================== Workflow Definition ===================== */

/// Workflow definition - represents a complete workflow file
///
/// Format: Optional front matter + top-level statements
///
/// Example:
/// ```text
/// name: my_workflow
/// description: A simple workflow
///
/// let x = 42
/// return x
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDef {
    /// Workflow body (statements to execute)
    pub body: Stmt,
    /// Optional YAML front matter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub front_matter: Option<String>,
}

/* ===================== PEST Parser ===================== */

#[derive(Parser)]
#[grammar = "parser/flow.pest"]
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
/// Accepts workflow syntax: Optional front matter + top-level statements
///
/// This is the production API. Use `parse()` for testing individual statements.
pub fn parse_workflow(source: &str) -> ParseResult<WorkflowDef> {
    let mut pairs = FlowParser::parse(Rule::program, source)?;

    let program = pairs.next().unwrap();

    // program = { SOI ~ (main_function | bare_workflow | statement) ~ EOI }
    let content = program.into_inner().next().unwrap();

    match content.as_rule() {
        Rule::main_function => build_main_function(content),
        Rule::bare_workflow => build_bare_workflow(content),
        Rule::statement => {
            // Reject bare statements - must have at least one statement (bare_workflow requires statement+)
            Err(ParseError::BuildError(
                "Workflow must contain top-level statements".to_string(),
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
///
/// Production code should use `parse_workflow`.
pub fn parse(source: &str) -> ParseResult<Stmt> {
    let mut pairs = FlowParser::parse(Rule::program, source)?;
    let program = pairs.next().unwrap();
    let content = program.into_inner().next().unwrap();

    match content.as_rule() {
        Rule::main_function => {
            // If it's a main function wrapper, extract the body
            let workflow = build_main_function(content)?;
            Ok(workflow.body)
        }
        Rule::bare_workflow => {
            // If it's a bare workflow, extract the body
            let workflow = build_bare_workflow(content)?;
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

fn build_bare_workflow(pair: pest::iterators::Pair<Rule>) -> ParseResult<WorkflowDef> {
    // bare_workflow = { front_matter? ~ statement+ }
    let inner = pair.into_inner();

    // Check for optional front matter
    let mut front_matter = None;
    let mut statements = Vec::new();

    for pair in inner {
        match pair.as_rule() {
            Rule::front_matter => {
                // Extract the front matter content
                let content_pair = pair.into_inner().next().unwrap();
                front_matter = Some(content_pair.as_str().to_string());
            }
            Rule::statement => {
                // Build each top-level statement
                statements.push(build_statement(pair)?);
            }
            _ => {
                return Err(ParseError::BuildError(format!(
                    "Unexpected bare_workflow content: {:?}",
                    pair.as_rule()
                )))
            }
        }
    }

    // Wrap all statements in a Block
    let body = Stmt::Block { body: statements };

    Ok(WorkflowDef { body, front_matter })
}

fn build_main_function(pair: pest::iterators::Pair<Rule>) -> ParseResult<WorkflowDef> {
    // main_function = { "async" ~ "function" ~ "main" ~ "(" ~ ")" ~ block }
    // This is syntax sugar - we simply unwrap the block and treat it as a bare workflow
    let mut inner = pair.into_inner();

    // Skip "async", "function", "main", "(", ")" tokens and get the block
    let block_pair = inner.next().unwrap();
    let body = build_block(block_pair)?;

    Ok(WorkflowDef {
        body,
        front_matter: None,
    })
}

fn build_block(pair: pest::iterators::Pair<Rule>) -> ParseResult<Stmt> {
    // block = { "{" ~ statement* ~ "}" }
    let statements: Result<Vec<Stmt>, ParseError> = pair
        .into_inner()
        .map(|stmt_pair| build_statement(stmt_pair))
        .collect();

    Ok(Stmt::Block { body: statements? })
}

fn build_if_stmt(pair: pest::iterators::Pair<Rule>) -> ParseResult<Stmt> {
    // if_stmt = { "if" ~ "(" ~ expression ~ ")" ~ block ~ else_clause? }
    let mut inner = pair.into_inner();

    // Get the test expression
    let test_pair = inner.next().unwrap();
    let test = build_expression(test_pair)?;

    // Get the then block
    let then_pair = inner.next().unwrap();
    let then_s = build_statement(then_pair)?;

    // Get optional else clause
    let else_s = if let Some(else_clause_pair) = inner.next() {
        // else_clause = { "else" ~ (if_stmt | block) }
        let else_inner = else_clause_pair.into_inner().next().unwrap();
        Some(Box::new(build_statement(else_inner)?))
    } else {
        None
    };

    Ok(Stmt::If {
        test,
        then_s: Box::new(then_s),
        else_s,
    })
}

fn build_while_stmt(pair: pest::iterators::Pair<Rule>) -> ParseResult<Stmt> {
    // while_stmt = { "while" ~ "(" ~ expression ~ ")" ~ block }
    let mut inner = pair.into_inner();

    // Get the test expression
    let test_pair = inner.next().unwrap();
    let test = build_expression(test_pair)?;

    // Get the body block
    let body_pair = inner.next().unwrap();
    let body = build_statement(body_pair)?;

    Ok(Stmt::While {
        test,
        body: Box::new(body),
    })
}

fn build_for_loop_stmt(pair: pest::iterators::Pair<Rule>) -> ParseResult<Stmt> {
    // for_loop_stmt = { "for" ~ "(" ~ var_kind ~ identifier ~ for_loop_kind ~ expression ~ ")" ~ block }
    let mut inner = pair.into_inner();

    // Get the var_kind (let or const)
    let kind_pair = inner.next().unwrap();
    let _var_kind = match kind_pair.as_str() {
        "let" => VarKind::Let,
        "const" => VarKind::Const,
        _ => {
            return Err(ParseError::BuildError(format!(
                "Expected 'let' or 'const', got: {}",
                kind_pair.as_str()
            )))
        }
    };

    // Get the binding identifier
    let binding = inner.next().unwrap().as_str().to_string();

    // Get the loop kind (in or of)
    let kind_pair = inner.next().unwrap();
    let kind = match kind_pair.as_str() {
        "of" => ForLoopKind::Of,
        "in" => ForLoopKind::In,
        _ => {
            return Err(ParseError::BuildError(format!(
                "Expected 'of' or 'in', got: {}",
                kind_pair.as_str()
            )))
        }
    };

    // Get the iterable expression
    let iterable_pair = inner.next().unwrap();
    let iterable = build_expression(iterable_pair)?;

    // Get the body block
    let body_pair = inner.next().unwrap();
    let body = build_statement(body_pair)?;

    Ok(Stmt::ForLoop {
        kind,
        binding,
        iterable,
        body: Box::new(body),
    })
}

fn build_declare_stmt(pair: pest::iterators::Pair<Rule>) -> ParseResult<Stmt> {
    // declare_stmt = { var_kind ~ declare_target ~ ("=" ~ expression)? }
    let mut inner = pair.into_inner();

    // Get the kind (let or const)
    let kind_pair = inner.next().unwrap();
    let var_kind = match kind_pair.as_str() {
        "let" => VarKind::Let,
        "const" => VarKind::Const,
        _ => {
            return Err(ParseError::BuildError(format!(
                "Expected 'let' or 'const', got: {}",
                kind_pair.as_str()
            )))
        }
    };

    // Get the declare target (identifier or destructure pattern)
    let target_pair = inner.next().unwrap();
    let target = build_declare_target(target_pair)?;

    // Get the optional initialization expression
    let init = if let Some(expr_pair) = inner.next() {
        Some(build_expression(expr_pair)?)
    } else {
        None
    };

    // Destructuring requires an initializer
    if matches!(target, DeclareTarget::Destructure { .. }) && init.is_none() {
        return Err(ParseError::BuildError(
            "Destructuring declaration requires an initializer".to_string(),
        ));
    }

    Ok(Stmt::Declare {
        var_kind,
        target,
        init,
    })
}

fn build_declare_target(pair: pest::iterators::Pair<Rule>) -> ParseResult<DeclareTarget> {
    // declare_target = { destructure_pattern | identifier }
    let inner = pair.into_inner().next().unwrap();

    match inner.as_rule() {
        Rule::identifier => Ok(DeclareTarget::Simple {
            name: inner.as_str().to_string(),
        }),
        Rule::destructure_pattern => {
            // destructure_pattern = { "{" ~ destructure_props ~ "}" }
            let props_pair = inner.into_inner().next().unwrap();
            let names: Vec<String> = props_pair
                .into_inner()
                .map(|id| id.as_str().to_string())
                .collect();
            Ok(DeclareTarget::Destructure { names })
        }
        _ => Err(ParseError::BuildError(format!(
            "Unexpected declare target rule: {:?}",
            inner.as_rule()
        ))),
    }
}

fn build_try_stmt(pair: pest::iterators::Pair<Rule>) -> ParseResult<Stmt> {
    // try_stmt = { "try" ~ block ~ "catch" ~ "(" ~ identifier ~ ")" ~ block }
    let mut inner = pair.into_inner();

    // Get the try body block
    let try_body_pair = inner.next().unwrap();
    let body = build_statement(try_body_pair)?;

    // Get the catch variable (identifier)
    let catch_var_pair = inner.next().unwrap();
    let catch_var = catch_var_pair.as_str().to_string();

    // Get the catch body block
    let catch_body_pair = inner.next().unwrap();
    let catch_body = build_statement(catch_body_pair)?;

    Ok(Stmt::Try {
        body: Box::new(body),
        catch_var,
        catch_body: Box::new(catch_body),
    })
}

fn build_assign_stmt(pair: pest::iterators::Pair<Rule>) -> ParseResult<Stmt> {
    // assign_stmt = { identifier ~ assign_path_segment* ~ "=" ~ expression }
    let mut inner = pair.into_inner();

    // Get the first identifier (variable name)
    let var = inner.next().unwrap().as_str().to_string();

    // Collect path segments (property and index access)
    let mut path = Vec::new();
    let mut expr_pair = None;

    for pair in inner {
        match pair.as_rule() {
            Rule::assign_path_segment => {
                // Process the path segment
                let segment_inner = pair.into_inner().next().unwrap();
                match segment_inner.as_rule() {
                    Rule::identifier => {
                        // Property access: .prop
                        path.push(MemberAccess::Prop {
                            property: segment_inner.as_str().to_string(),
                        });
                    }
                    Rule::expression => {
                        // Index access: [expr]
                        let index_expr = build_expression(segment_inner)?;
                        path.push(MemberAccess::Index { expr: index_expr });
                    }
                    _ => {}
                }
            }
            Rule::expression => {
                // This is the value expression (right-hand side of =)
                expr_pair = Some(pair);
                break;
            }
            _ => {}
        }
    }

    // Build the value expression
    let value = build_expression(expr_pair.unwrap())?;

    Ok(Stmt::Assign { var, path, value })
}

fn build_binary_expr(pair: pest::iterators::Pair<Rule>) -> ParseResult<Expr> {
    let inner_pairs: Vec<_> = pair.into_inner().collect();

    if inner_pairs.is_empty() {
        return Err(ParseError::BuildError(
            "Empty binary expression".to_string(),
        ));
    }

    // Get the first operand
    let mut left = build_expression(inner_pairs[0].clone())?;

    // Process remaining pairs (operator, operand, operator, operand, ...)
    let mut i = 1;
    while i < inner_pairs.len() {
        // Get operator (as a named rule)
        let op_rule = inner_pairs[i].as_rule();

        // Get the right operand
        i += 1;
        if i >= inner_pairs.len() {
            return Err(ParseError::BuildError(
                "Missing right operand after operator".to_string(),
            ));
        }

        let right = build_expression(inner_pairs[i].clone())?;

        // For &&, ||, and ??, create BinaryOp nodes for short-circuit evaluation
        // For other operators, desugar to function calls
        left = match op_rule {
            Rule::op_and => Expr::BinaryOp {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
            },
            Rule::op_or => Expr::BinaryOp {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            },
            Rule::op_nullish => Expr::BinaryOp {
                op: BinaryOp::Nullish,
                left: Box::new(left),
                right: Box::new(right),
            },
            _ => {
                // Map other operators to function calls
                let func_name = match op_rule {
                    Rule::op_eq => "eq",
                    Rule::op_ne => "ne",
                    Rule::op_lt => "lt",
                    Rule::op_lte => "lte",
                    Rule::op_gt => "gt",
                    Rule::op_gte => "gte",
                    Rule::op_add => "add",
                    Rule::op_sub => "sub",
                    Rule::op_mul => "mul",
                    Rule::op_div => "div",
                    _ => {
                        return Err(ParseError::BuildError(format!(
                            "Expected operator rule at index {}, got {:?}",
                            i - 1,
                            op_rule
                        )));
                    }
                };

                Expr::Call {
                    callee: Box::new(Expr::Ident {
                        name: func_name.to_string(),
                    }),
                    args: vec![left, right],
                }
            }
        };

        i += 1;
    }

    Ok(left)
}

fn build_statement(pair: pest::iterators::Pair<Rule>) -> ParseResult<Stmt> {
    match pair.as_rule() {
        Rule::statement => {
            // statement = { return_stmt | if_stmt | while_stmt | try_stmt | break_stmt | continue_stmt | block | declare_stmt | assign_stmt | expr_stmt }
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
        Rule::if_stmt => {
            // if_stmt = { "if" ~ "(" ~ expression ~ ")" ~ block ~ else_clause? }
            build_if_stmt(pair)
        }
        Rule::while_stmt => {
            // while_stmt = { "while" ~ "(" ~ expression ~ ")" ~ block }
            build_while_stmt(pair)
        }
        Rule::for_loop_stmt => {
            // for_loop_stmt = { "for" ~ "(" ~ var_kind ~ identifier ~ for_loop_kind ~ expression ~ ")" ~ block }
            build_for_loop_stmt(pair)
        }
        Rule::try_stmt => {
            // try_stmt = { "try" ~ block ~ "catch" ~ "(" ~ identifier ~ ")" ~ block }
            build_try_stmt(pair)
        }
        Rule::break_stmt => {
            // break_stmt = { "break" }
            Ok(Stmt::Break)
        }
        Rule::continue_stmt => {
            // continue_stmt = { "continue" }
            Ok(Stmt::Continue)
        }
        Rule::block => {
            // block = { "{" ~ statement* ~ "}" }
            build_block(pair)
        }
        Rule::declare_stmt => {
            // declare_stmt = { ("let" | "const") ~ identifier ~ "=" ~ expression }
            build_declare_stmt(pair)
        }
        Rule::assign_stmt => {
            // assign_stmt = { identifier ~ assign_path_segment* ~ "=" ~ expression }
            build_assign_stmt(pair)
        }
        Rule::expr_stmt => {
            // expr_stmt = { expression }
            let expr_pair = pair.into_inner().next().unwrap();
            let expr = build_expression(expr_pair)?;
            Ok(Stmt::Expr { expr })
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
            // expression = { await_expr | ternary_expr }
            let inner = pair.into_inner().next().unwrap();
            build_expression(inner)
        }
        Rule::ternary_expr => {
            // ternary_expr = { nullish_expr ~ ("?" ~ expression ~ ":" ~ expression)? }
            let mut inner = pair.into_inner();
            let condition_pair = inner.next().unwrap();
            let condition = build_expression(condition_pair)?;

            // Check if there's a ternary part (? then : else)
            if let Some(consequent_pair) = inner.next() {
                let consequent = build_expression(consequent_pair)?;
                let alternate_pair = inner.next().unwrap();
                let alternate = build_expression(alternate_pair)?;
                Ok(Expr::Ternary {
                    condition: Box::new(condition),
                    consequent: Box::new(consequent),
                    alternate: Box::new(alternate),
                })
            } else {
                // No ternary part, just return the condition
                Ok(condition)
            }
        }
        Rule::nullish_expr => build_binary_expr(pair),
        Rule::logical_or_expr => build_binary_expr(pair),
        Rule::logical_and_expr => build_binary_expr(pair),
        Rule::equality_expr => build_binary_expr(pair),
        Rule::comparison_expr => build_binary_expr(pair),
        Rule::additive_expr => build_binary_expr(pair),
        Rule::multiplicative_expr => build_binary_expr(pair),
        Rule::unary_expr => {
            // unary_expr = { op_not ~ unary_expr | call_expr }
            let mut inner = pair.into_inner();
            let first = inner.next().unwrap();

            match first.as_rule() {
                Rule::op_not => {
                    // This is !expr - build it as a function call: not(expr)
                    let operand_pair = inner.next().unwrap();
                    let operand = build_expression(operand_pair)?;
                    Ok(Expr::Call {
                        callee: Box::new(Expr::Ident {
                            name: "not".to_string(),
                        }),
                        args: vec![operand],
                    })
                }
                _ => {
                    // This is just a call_expr
                    build_expression(first)
                }
            }
        }
        Rule::await_expr => {
            // await_expr = { "await" ~ expression }
            // The "await" keyword is consumed by the grammar, only expression remains
            let mut inner = pair.into_inner();
            let expr_pair = inner.next().unwrap();
            let inner_expr = build_expression(expr_pair)?;
            Ok(Expr::Await {
                inner: Box::new(inner_expr),
            })
        }
        Rule::call_expr => {
            // call_expr = { member_expr ~ call_suffix? }
            let mut inner = pair.into_inner();

            // Build the member expression (callee)
            let member_pair = inner.next().unwrap();
            let mut expr = build_expression(member_pair)?;

            // Check if there's a call suffix
            if let Some(call_suffix_pair) = inner.next() {
                // This is a function call - extract args from call_suffix
                let mut suffix_inner = call_suffix_pair.into_inner();

                let args = if let Some(arg_list_pair) = suffix_inner.next() {
                    build_arg_list(arg_list_pair)?
                } else {
                    // Empty argument list
                    vec![]
                };

                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                };
            }

            Ok(expr)
        }
        Rule::member_expr => {
            // member_expr = { primary ~ (member_access)* }
            let mut inner = pair.into_inner();

            // Start with the primary expression
            let primary = inner.next().unwrap();
            let mut expr = build_expression(primary)?;

            // Chain member accesses left-to-right
            for access_pair in inner {
                // member_access = { optional_access | regular_access }
                let access_inner = access_pair.into_inner().next().unwrap();
                let (optional, property) = match access_inner.as_rule() {
                    Rule::optional_access => {
                        // optional_access = { "?." ~ identifier }
                        let prop = access_inner
                            .into_inner()
                            .next()
                            .unwrap()
                            .as_str()
                            .to_string();
                        (true, prop)
                    }
                    Rule::regular_access => {
                        // regular_access = { "." ~ identifier }
                        let prop = access_inner
                            .into_inner()
                            .next()
                            .unwrap()
                            .as_str()
                            .to_string();
                        (false, prop)
                    }
                    _ => unreachable!("Unexpected member access rule"),
                };

                expr = Expr::Member {
                    object: Box::new(expr),
                    property,
                    optional,
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
        Rule::object_lit => {
            // object_lit = { "{" ~ property_list? ~ "}" }
            build_object_literal(pair)
        }
        Rule::array_lit => {
            // array_lit = { "[" ~ element_list? ~ "]" }
            build_array_literal(pair)
        }
        _ => Err(ParseError::BuildError(format!(
            "Unexpected expression rule: {:?}",
            pair.as_rule()
        ))),
    }
}

fn build_arg_list(pair: pest::iterators::Pair<Rule>) -> ParseResult<Vec<Expr>> {
    // arg_list = { expression ~ ("," ~ expression)* }
    let args: Result<Vec<Expr>, ParseError> = pair
        .into_inner()
        .map(|expr_pair| build_expression(expr_pair))
        .collect();
    args
}

fn build_object_literal(pair: pest::iterators::Pair<Rule>) -> ParseResult<Expr> {
    // object_lit = { "{" ~ property_list? ~ "}" }
    let mut inner = pair.into_inner();

    let properties = if let Some(property_list_pair) = inner.next() {
        // Has properties - build the property list
        build_property_list(property_list_pair)?
    } else {
        // Empty object
        vec![]
    };

    Ok(Expr::LitObj { properties })
}

fn build_property_list(pair: pest::iterators::Pair<Rule>) -> ParseResult<Vec<(String, Expr)>> {
    // property_list = { property ~ ("," ~ property)* ~ ","? }
    let properties: Result<Vec<(String, Expr)>, ParseError> = pair
        .into_inner()
        .map(|property_pair| build_property(property_pair))
        .collect();
    properties
}

fn build_property(pair: pest::iterators::Pair<Rule>) -> ParseResult<(String, Expr)> {
    // property = { property_pair | property_shorthand }
    let inner = pair.into_inner().next().unwrap();

    match inner.as_rule() {
        Rule::property_pair => {
            // property_pair = { identifier ~ ":" ~ expression }
            let mut inner_pairs = inner.into_inner();
            let key = inner_pairs.next().unwrap().as_str().to_string();
            let value_pair = inner_pairs.next().unwrap();
            let value = build_expression(value_pair)?;
            Ok((key, value))
        }
        Rule::property_shorthand => {
            // property_shorthand = { identifier }
            // Expands to { key: key } where value is an identifier reference
            let key = inner.as_str().to_string();
            let value = Expr::Ident { name: key.clone() };
            Ok((key, value))
        }
        _ => Err(ParseError::BuildError(format!(
            "Unexpected property rule: {:?}",
            inner.as_rule()
        ))),
    }
}

fn build_array_literal(pair: pest::iterators::Pair<Rule>) -> ParseResult<Expr> {
    // array_lit = { "[" ~ element_list? ~ "]" }
    let mut inner = pair.into_inner();

    let elements = if let Some(element_list_pair) = inner.next() {
        // Has elements - build the element list
        build_element_list(element_list_pair)?
    } else {
        // Empty array
        vec![]
    };

    Ok(Expr::LitList { elements })
}

fn build_element_list(pair: pest::iterators::Pair<Rule>) -> ParseResult<Vec<Expr>> {
    // element_list = { expression ~ ("," ~ expression)* ~ ","? }
    let elements: Result<Vec<Expr>, ParseError> = pair
        .into_inner()
        .map(|expr_pair| build_expression(expr_pair))
        .collect();
    elements
}
