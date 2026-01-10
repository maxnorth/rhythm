//! Parser v2 - PEST-based parser for Flow language
//!
//! Produces AST compatible with executor_v2, with span information for error reporting.

use pest::Parser;
use pest_derive::Parser;
use serde::{Deserialize, Serialize};

use super::executor::types::ast::{
    BinaryOp, DeclareTarget, Expr, ForLoopKind, MemberAccess, Span, Stmt, VarKind,
};

pub mod semantic_validator;

#[cfg(test)]
mod tests;

/* ===================== Workflow Definition ===================== */

/// Workflow definition - represents a complete workflow file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDef {
    /// Workflow body (statements to execute)
    pub body: Stmt,
    /// Optional YAML front matter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub front_matter: Option<String>,
    /// Span of the entire workflow
    #[serde(default, skip_serializing_if = "is_default_span")]
    pub span: Span,
}

fn is_default_span(span: &Span) -> bool {
    *span == Span::default()
}

/* ===================== PEST Parser ===================== */

#[derive(Parser)]
#[grammar = "parser/flow.pest"]
struct FlowParser;

/* ===================== Error Types ===================== */

#[derive(Debug)]
pub enum ParseError {
    PestError(String, Option<Span>),
    BuildError(String, Option<Span>),
}

impl ParseError {
    pub fn span(&self) -> Option<Span> {
        match self {
            ParseError::PestError(_, span) => *span,
            ParseError::BuildError(_, span) => *span,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            ParseError::PestError(msg, _) => msg,
            ParseError::BuildError(msg, _) => msg,
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::PestError(msg, _) => write!(f, "{}", msg),
            ParseError::BuildError(msg, _) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<pest::error::Error<Rule>> for ParseError {
    fn from(err: pest::error::Error<Rule>) -> Self {
        let span = match err.line_col {
            pest::error::LineColLocation::Pos((line, col)) => Some(Span {
                start: 0,
                end: 0,
                start_line: line.saturating_sub(1),
                start_col: col.saturating_sub(1),
                end_line: line.saturating_sub(1),
                end_col: col,
            }),
            pest::error::LineColLocation::Span((start_line, start_col), (end_line, end_col)) => {
                Some(Span {
                    start: 0,
                    end: 0,
                    start_line: start_line.saturating_sub(1),
                    start_col: start_col.saturating_sub(1),
                    end_line: end_line.saturating_sub(1),
                    end_col: end_col.saturating_sub(1),
                })
            }
        };
        ParseError::PestError(err.to_string(), span)
    }
}

pub type ParseResult<T> = Result<T, ParseError>;

/* ===================== Span Helpers ===================== */

/// Convert a PEST pair's span to our Span type
fn pair_to_span(pair: &pest::iterators::Pair<Rule>, source: &str) -> Span {
    let pest_span = pair.as_span();
    let start = pest_span.start();
    let end = pest_span.end();

    let (start_line, start_col) = offset_to_line_col(source, start);
    let (end_line, end_col) = offset_to_line_col(source, end);

    Span::new(start, end, start_line, start_col, end_line, end_col)
}

/// Convert byte offset to (line, column) - 0-indexed
fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    let mut current_offset = 0;

    for ch in source.chars() {
        if current_offset >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
        current_offset += ch.len_utf8();
    }

    (line, col)
}

/* ===================== Public API ===================== */

/// Parse a Flow source string into a workflow definition
pub fn parse_workflow(source: &str) -> ParseResult<WorkflowDef> {
    let mut pairs = FlowParser::parse(Rule::program, source)?;

    let program = pairs.next().unwrap();
    let program_span = pair_to_span(&program, source);
    let content = program.into_inner().next().unwrap();

    match content.as_rule() {
        Rule::main_function => build_main_function(content, source, program_span),
        Rule::bare_workflow => build_bare_workflow(content, source, program_span),
        Rule::statement => Err(ParseError::BuildError(
            "Workflow must contain top-level statements".to_string(),
            Some(program_span),
        )),
        _ => Err(ParseError::BuildError(
            format!("Unexpected program content: {:?}", content.as_rule()),
            Some(program_span),
        )),
    }
}

/// Parse a Flow source string into an AST statement (testing API)
pub fn parse(source: &str) -> ParseResult<Stmt> {
    let mut pairs = FlowParser::parse(Rule::program, source)?;
    let program = pairs.next().unwrap();
    let program_span = pair_to_span(&program, source);
    let content = program.into_inner().next().unwrap();

    match content.as_rule() {
        Rule::main_function => {
            let workflow = build_main_function(content, source, program_span)?;
            Ok(workflow.body)
        }
        Rule::bare_workflow => {
            let workflow = build_bare_workflow(content, source, program_span)?;
            Ok(workflow.body)
        }
        Rule::statement => build_statement(content, source),
        _ => Err(ParseError::BuildError(
            format!("Unexpected program content: {:?}", content.as_rule()),
            Some(program_span),
        )),
    }
}

/* ===================== AST Builder ===================== */

fn build_bare_workflow(
    pair: pest::iterators::Pair<Rule>,
    source: &str,
    program_span: Span,
) -> ParseResult<WorkflowDef> {
    let inner = pair.into_inner();
    let mut front_matter = None;
    let mut statements = Vec::new();

    for pair in inner {
        match pair.as_rule() {
            Rule::front_matter => {
                let content_pair = pair.into_inner().next().unwrap();
                front_matter = Some(content_pair.as_str().to_string());
            }
            Rule::statement => {
                statements.push(build_statement(pair, source)?);
            }
            _ => {
                return Err(ParseError::BuildError(
                    format!("Unexpected bare_workflow content: {:?}", pair.as_rule()),
                    None,
                ))
            }
        }
    }

    let body_span = if statements.is_empty() {
        program_span
    } else {
        statements
            .first()
            .unwrap()
            .span()
            .merge(&statements.last().unwrap().span())
    };

    let body = Stmt::Block {
        body: statements,
        span: body_span,
    };
    Ok(WorkflowDef {
        body,
        front_matter,
        span: program_span,
    })
}

fn build_main_function(
    pair: pest::iterators::Pair<Rule>,
    source: &str,
    program_span: Span,
) -> ParseResult<WorkflowDef> {
    let mut inner = pair.into_inner();
    let block_pair = inner.next().unwrap();
    let body = build_block(block_pair, source)?;

    Ok(WorkflowDef {
        body,
        front_matter: None,
        span: program_span,
    })
}

fn build_block(pair: pest::iterators::Pair<Rule>, source: &str) -> ParseResult<Stmt> {
    let span = pair_to_span(&pair, source);
    let statements: Result<Vec<Stmt>, ParseError> = pair
        .into_inner()
        .map(|stmt_pair| build_statement(stmt_pair, source))
        .collect();

    Ok(Stmt::Block {
        body: statements?,
        span,
    })
}

fn build_if_stmt(pair: pest::iterators::Pair<Rule>, source: &str) -> ParseResult<Stmt> {
    let span = pair_to_span(&pair, source);
    let mut inner = pair.into_inner();

    let test_pair = inner.next().unwrap();
    let test = build_expression(test_pair, source)?;

    let then_pair = inner.next().unwrap();
    let then_s = build_statement(then_pair, source)?;

    let else_s = if let Some(else_clause_pair) = inner.next() {
        let else_inner = else_clause_pair.into_inner().next().unwrap();
        Some(Box::new(build_statement(else_inner, source)?))
    } else {
        None
    };

    Ok(Stmt::If {
        test,
        then_s: Box::new(then_s),
        else_s,
        span,
    })
}

fn build_while_stmt(pair: pest::iterators::Pair<Rule>, source: &str) -> ParseResult<Stmt> {
    let span = pair_to_span(&pair, source);
    let mut inner = pair.into_inner();

    let test_pair = inner.next().unwrap();
    let test = build_expression(test_pair, source)?;

    let body_pair = inner.next().unwrap();
    let body = build_statement(body_pair, source)?;

    Ok(Stmt::While {
        test,
        body: Box::new(body),
        span,
    })
}

fn build_for_loop_stmt(pair: pest::iterators::Pair<Rule>, source: &str) -> ParseResult<Stmt> {
    let span = pair_to_span(&pair, source);
    let mut inner = pair.into_inner();

    let kind_pair = inner.next().unwrap();
    let _var_kind = match kind_pair.as_str() {
        "let" => VarKind::Let,
        "const" => VarKind::Const,
        _ => {
            return Err(ParseError::BuildError(
                format!("Expected 'let' or 'const', got: {}", kind_pair.as_str()),
                Some(pair_to_span(&kind_pair, source)),
            ))
        }
    };

    let binding_pair = inner.next().unwrap();
    let binding_span = pair_to_span(&binding_pair, source);
    let binding = binding_pair.as_str().to_string();

    let kind_pair = inner.next().unwrap();
    let kind = match kind_pair.as_str() {
        "of" => ForLoopKind::Of,
        "in" => ForLoopKind::In,
        _ => {
            return Err(ParseError::BuildError(
                format!("Expected 'of' or 'in', got: {}", kind_pair.as_str()),
                Some(pair_to_span(&kind_pair, source)),
            ))
        }
    };

    let iterable_pair = inner.next().unwrap();
    let iterable = build_expression(iterable_pair, source)?;

    let body_pair = inner.next().unwrap();
    let body = build_statement(body_pair, source)?;

    Ok(Stmt::ForLoop {
        kind,
        binding,
        binding_span,
        iterable,
        body: Box::new(body),
        span,
    })
}

fn build_declare_stmt(pair: pest::iterators::Pair<Rule>, source: &str) -> ParseResult<Stmt> {
    let span = pair_to_span(&pair, source);
    let mut inner = pair.into_inner();

    let kind_pair = inner.next().unwrap();
    let var_kind = match kind_pair.as_str() {
        "let" => VarKind::Let,
        "const" => VarKind::Const,
        _ => {
            return Err(ParseError::BuildError(
                format!("Expected 'let' or 'const', got: {}", kind_pair.as_str()),
                Some(pair_to_span(&kind_pair, source)),
            ))
        }
    };

    let target_pair = inner.next().unwrap();
    let target = build_declare_target(target_pair, source)?;

    let init = if let Some(expr_pair) = inner.next() {
        Some(build_expression(expr_pair, source)?)
    } else {
        None
    };

    if matches!(target, DeclareTarget::Destructure { .. }) && init.is_none() {
        return Err(ParseError::BuildError(
            "Destructuring declaration requires an initializer".to_string(),
            Some(span),
        ));
    }

    Ok(Stmt::Declare {
        var_kind,
        target,
        init,
        span,
    })
}

fn build_declare_target(
    pair: pest::iterators::Pair<Rule>,
    source: &str,
) -> ParseResult<DeclareTarget> {
    let inner = pair.into_inner().next().unwrap();
    let inner_span = pair_to_span(&inner, source);

    match inner.as_rule() {
        Rule::identifier => Ok(DeclareTarget::Simple {
            name: inner.as_str().to_string(),
            span: inner_span,
        }),
        Rule::destructure_pattern => {
            let props_pair = inner.into_inner().next().unwrap();
            let mut names = Vec::new();
            let mut spans = Vec::new();
            for id in props_pair.into_inner() {
                names.push(id.as_str().to_string());
                spans.push(pair_to_span(&id, source));
            }
            Ok(DeclareTarget::Destructure {
                names,
                spans,
                span: inner_span,
            })
        }
        _ => Err(ParseError::BuildError(
            format!("Unexpected declare target rule: {:?}", inner.as_rule()),
            Some(inner_span),
        )),
    }
}

fn build_try_stmt(pair: pest::iterators::Pair<Rule>, source: &str) -> ParseResult<Stmt> {
    let span = pair_to_span(&pair, source);
    let mut inner = pair.into_inner();

    let try_body_pair = inner.next().unwrap();
    let body = build_statement(try_body_pair, source)?;

    let catch_var_pair = inner.next().unwrap();
    let catch_var_span = pair_to_span(&catch_var_pair, source);
    let catch_var = catch_var_pair.as_str().to_string();

    let catch_body_pair = inner.next().unwrap();
    let catch_body = build_statement(catch_body_pair, source)?;

    Ok(Stmt::Try {
        body: Box::new(body),
        catch_var,
        catch_var_span,
        catch_body: Box::new(catch_body),
        span,
    })
}

fn build_assign_stmt(pair: pest::iterators::Pair<Rule>, source: &str) -> ParseResult<Stmt> {
    let span = pair_to_span(&pair, source);
    let mut inner = pair.into_inner();

    let var_pair = inner.next().unwrap();
    let var_span = pair_to_span(&var_pair, source);
    let var = var_pair.as_str().to_string();

    let mut path = Vec::new();
    let mut expr_pair = None;

    for pair in inner {
        match pair.as_rule() {
            Rule::assign_path_segment => {
                let segment_span = pair_to_span(&pair, source);
                let segment_inner = pair.into_inner().next().unwrap();
                match segment_inner.as_rule() {
                    Rule::identifier => {
                        path.push(MemberAccess::Prop {
                            property: segment_inner.as_str().to_string(),
                            span: segment_span,
                        });
                    }
                    Rule::expression => {
                        let index_expr = build_expression(segment_inner, source)?;
                        path.push(MemberAccess::Index {
                            expr: index_expr,
                            span: segment_span,
                        });
                    }
                    _ => {}
                }
            }
            Rule::expression => {
                expr_pair = Some(pair);
                break;
            }
            _ => {}
        }
    }

    let value = build_expression(expr_pair.unwrap(), source)?;
    Ok(Stmt::Assign {
        var,
        var_span,
        path,
        value,
        span,
    })
}

fn build_binary_expr(pair: pest::iterators::Pair<Rule>, source: &str) -> ParseResult<Expr> {
    let span = pair_to_span(&pair, source);
    let inner_pairs: Vec<_> = pair.into_inner().collect();

    if inner_pairs.is_empty() {
        return Err(ParseError::BuildError(
            "Empty binary expression".to_string(),
            Some(span),
        ));
    }

    let mut left = build_expression(inner_pairs[0].clone(), source)?;

    let mut i = 1;
    while i < inner_pairs.len() {
        let op_rule = inner_pairs[i].as_rule();

        i += 1;
        if i >= inner_pairs.len() {
            return Err(ParseError::BuildError(
                "Missing right operand after operator".to_string(),
                Some(span),
            ));
        }

        let right = build_expression(inner_pairs[i].clone(), source)?;
        let new_span = left.span().merge(&right.span());

        left = match op_rule {
            Rule::op_and => Expr::BinaryOp {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
                span: new_span,
            },
            Rule::op_or => Expr::BinaryOp {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
                span: new_span,
            },
            Rule::op_nullish => Expr::BinaryOp {
                op: BinaryOp::Nullish,
                left: Box::new(left),
                right: Box::new(right),
                span: new_span,
            },
            _ => {
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
                        return Err(ParseError::BuildError(
                            format!(
                                "Expected operator rule at index {}, got {:?}",
                                i - 1,
                                op_rule
                            ),
                            Some(span),
                        ))
                    }
                };

                Expr::Call {
                    callee: Box::new(Expr::Ident {
                        name: func_name.to_string(),
                        span: new_span,
                    }),
                    args: vec![left, right],
                    span: new_span,
                }
            }
        };

        i += 1;
    }

    Ok(left)
}

fn build_statement(pair: pest::iterators::Pair<Rule>, source: &str) -> ParseResult<Stmt> {
    let span = pair_to_span(&pair, source);

    match pair.as_rule() {
        Rule::statement => {
            let inner = pair.into_inner().next().unwrap();
            build_statement(inner, source)
        }
        Rule::return_stmt => {
            let mut inner = pair.into_inner();
            let expr_pair = inner.next().unwrap();
            let expr = build_expression(expr_pair, source)?;
            Ok(Stmt::Return {
                value: Some(expr),
                span,
            })
        }
        Rule::if_stmt => build_if_stmt(pair, source),
        Rule::while_stmt => build_while_stmt(pair, source),
        Rule::for_loop_stmt => build_for_loop_stmt(pair, source),
        Rule::try_stmt => build_try_stmt(pair, source),
        Rule::break_stmt => Ok(Stmt::Break { span }),
        Rule::continue_stmt => Ok(Stmt::Continue { span }),
        Rule::block => build_block(pair, source),
        Rule::declare_stmt => build_declare_stmt(pair, source),
        Rule::assign_stmt => build_assign_stmt(pair, source),
        Rule::expr_stmt => {
            let expr_pair = pair.into_inner().next().unwrap();
            let expr = build_expression(expr_pair, source)?;
            Ok(Stmt::Expr { expr, span })
        }
        _ => Err(ParseError::BuildError(
            format!("Unexpected statement rule: {:?}", pair.as_rule()),
            Some(span),
        )),
    }
}

fn build_expression(pair: pest::iterators::Pair<Rule>, source: &str) -> ParseResult<Expr> {
    let span = pair_to_span(&pair, source);

    match pair.as_rule() {
        Rule::expression => {
            let inner = pair.into_inner().next().unwrap();
            build_expression(inner, source)
        }
        Rule::ternary_expr => {
            let mut inner = pair.into_inner();
            let condition_pair = inner.next().unwrap();
            let condition = build_expression(condition_pair, source)?;

            if let Some(consequent_pair) = inner.next() {
                let consequent = build_expression(consequent_pair, source)?;
                let alternate_pair = inner.next().unwrap();
                let alternate = build_expression(alternate_pair, source)?;
                Ok(Expr::Ternary {
                    condition: Box::new(condition),
                    consequent: Box::new(consequent),
                    alternate: Box::new(alternate),
                    span,
                })
            } else {
                Ok(condition)
            }
        }
        Rule::nullish_expr
        | Rule::logical_or_expr
        | Rule::logical_and_expr
        | Rule::equality_expr
        | Rule::comparison_expr
        | Rule::additive_expr
        | Rule::multiplicative_expr => build_binary_expr(pair, source),
        Rule::unary_expr => {
            let mut inner = pair.into_inner();
            let first = inner.next().unwrap();

            match first.as_rule() {
                Rule::op_not => {
                    let operand_pair = inner.next().unwrap();
                    let operand = build_expression(operand_pair, source)?;
                    Ok(Expr::Call {
                        callee: Box::new(Expr::Ident {
                            name: "not".to_string(),
                            span,
                        }),
                        args: vec![operand],
                        span,
                    })
                }
                _ => build_expression(first, source),
            }
        }
        Rule::await_expr => {
            let mut inner = pair.into_inner();
            let expr_pair = inner.next().unwrap();
            let inner_expr = build_expression(expr_pair, source)?;
            Ok(Expr::Await {
                inner: Box::new(inner_expr),
                span,
            })
        }
        Rule::call_expr => {
            let mut inner = pair.into_inner();
            let primary_pair = inner.next().unwrap();
            let mut expr = build_expression(primary_pair, source)?;

            for postfix_pair in inner {
                let postfix_span = pair_to_span(&postfix_pair, source);
                let postfix_inner = postfix_pair.into_inner().next().unwrap();

                match postfix_inner.as_rule() {
                    Rule::call_suffix => {
                        let mut suffix_inner = postfix_inner.into_inner();
                        let args = if let Some(arg_list_pair) = suffix_inner.next() {
                            build_arg_list(arg_list_pair, source)?
                        } else {
                            vec![]
                        };
                        let new_span = expr.span().merge(&postfix_span);
                        expr = Expr::Call {
                            callee: Box::new(expr),
                            args,
                            span: new_span,
                        };
                    }
                    Rule::optional_access => {
                        let prop_pair = postfix_inner.into_inner().next().unwrap();
                        let property_span = pair_to_span(&prop_pair, source);
                        let prop = prop_pair.as_str().to_string();
                        let new_span = expr.span().merge(&postfix_span);
                        expr = Expr::Member {
                            object: Box::new(expr),
                            property: prop,
                            property_span,
                            optional: true,
                            span: new_span,
                        };
                    }
                    Rule::regular_access => {
                        let prop_pair = postfix_inner.into_inner().next().unwrap();
                        let property_span = pair_to_span(&prop_pair, source);
                        let prop = prop_pair.as_str().to_string();
                        let new_span = expr.span().merge(&postfix_span);
                        expr = Expr::Member {
                            object: Box::new(expr),
                            property: prop,
                            property_span,
                            optional: false,
                            span: new_span,
                        };
                    }
                    _ => unreachable!("Unexpected postfix rule: {:?}", postfix_inner.as_rule()),
                }
            }

            Ok(expr)
        }
        Rule::primary => {
            let inner = pair.into_inner().next().unwrap();
            build_expression(inner, source)
        }
        Rule::identifier => {
            let name = pair.as_str().to_string();
            Ok(Expr::Ident { name, span })
        }
        Rule::literal => {
            let inner = pair.into_inner().next().unwrap();
            build_expression(inner, source)
        }
        Rule::number => {
            let num_str = pair.as_str();
            let value = num_str.parse::<f64>().map_err(|e| {
                ParseError::BuildError(
                    format!("Failed to parse number '{}': {}", num_str, e),
                    Some(span),
                )
            })?;
            Ok(Expr::LitNum { v: value, span })
        }
        Rule::boolean => {
            let value = pair.as_str() == "true";
            Ok(Expr::LitBool { v: value, span })
        }
        Rule::string => {
            let mut inner = pair.into_inner();
            let content = inner.next().unwrap();
            let value = content.as_str().to_string();
            Ok(Expr::LitStr { v: value, span })
        }
        Rule::null_lit => Ok(Expr::LitNull { span }),
        Rule::object_lit => build_object_literal(pair, source),
        Rule::array_lit => build_array_literal(pair, source),
        _ => Err(ParseError::BuildError(
            format!("Unexpected expression rule: {:?}", pair.as_rule()),
            Some(span),
        )),
    }
}

fn build_arg_list(pair: pest::iterators::Pair<Rule>, source: &str) -> ParseResult<Vec<Expr>> {
    pair.into_inner()
        .map(|expr_pair| build_expression(expr_pair, source))
        .collect()
}

fn build_object_literal(pair: pest::iterators::Pair<Rule>, source: &str) -> ParseResult<Expr> {
    let span = pair_to_span(&pair, source);
    let mut inner = pair.into_inner();

    let properties = if let Some(property_list_pair) = inner.next() {
        build_property_list(property_list_pair, source)?
    } else {
        vec![]
    };

    Ok(Expr::LitObj { properties, span })
}

fn build_property_list(
    pair: pest::iterators::Pair<Rule>,
    source: &str,
) -> ParseResult<Vec<(String, Span, Expr)>> {
    pair.into_inner()
        .map(|property_pair| build_property(property_pair, source))
        .collect()
}

fn build_property(
    pair: pest::iterators::Pair<Rule>,
    source: &str,
) -> ParseResult<(String, Span, Expr)> {
    let inner = pair.into_inner().next().unwrap();
    let inner_span = pair_to_span(&inner, source);

    match inner.as_rule() {
        Rule::property_pair => {
            let mut inner_pairs = inner.into_inner();
            let key_pair = inner_pairs.next().unwrap();
            let key_span = pair_to_span(&key_pair, source);
            let key = key_pair.as_str().to_string();
            let value_pair = inner_pairs.next().unwrap();
            let value = build_expression(value_pair, source)?;
            Ok((key, key_span, value))
        }
        Rule::property_shorthand => {
            let key = inner.as_str().to_string();
            let value = Expr::Ident {
                name: key.clone(),
                span: inner_span,
            };
            Ok((key, inner_span, value))
        }
        _ => Err(ParseError::BuildError(
            format!("Unexpected property rule: {:?}", inner.as_rule()),
            Some(inner_span),
        )),
    }
}

fn build_array_literal(pair: pest::iterators::Pair<Rule>, source: &str) -> ParseResult<Expr> {
    let span = pair_to_span(&pair, source);
    let mut inner = pair.into_inner();

    let elements = if let Some(element_list_pair) = inner.next() {
        build_element_list(element_list_pair, source)?
    } else {
        vec![]
    };

    Ok(Expr::LitList { elements, span })
}

fn build_element_list(pair: pest::iterators::Pair<Rule>, source: &str) -> ParseResult<Vec<Expr>> {
    pair.into_inner()
        .map(|expr_pair| build_expression(expr_pair, source))
        .collect()
}
