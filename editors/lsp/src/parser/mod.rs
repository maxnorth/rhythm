//! Parser for Flow language with span tracking
//!
//! This module wraps rhythm-core's parser and provides LSP-specific types
//! with full span information.

pub mod ast;

pub use ast::{DeclareTarget, Expr, ExprKind, Span, Spanned, Stmt, StmtKind, VarKind, WorkflowDef};

use ast::*;

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

/// Parse a Flow source string into a workflow definition
///
/// This function calls rhythm-core's parser and converts the result to
/// the LSP's AST format, which includes additional span information.
pub fn parse_workflow(source: &str) -> ParseResult<WorkflowDef> {
    match rhythm_core::parser::parse_workflow(source) {
        Ok(core_wf) => Ok(convert_workflow(&core_wf)),
        Err(e) => {
            let span = e.span().map(convert_span);
            Err(ParseError {
                message: e.to_string(),
                span,
            })
        }
    }
}

// ============================================================================
// Conversion from rhythm-core AST to LSP AST
// ============================================================================

/// Convert a span from core to LSP format
fn convert_span(span: rhythm_core::executor::types::ast::Span) -> Span {
    Span {
        start: span.start,
        end: span.end,
        start_line: span.start_line,
        start_col: span.start_col,
        end_line: span.end_line,
        end_col: span.end_col,
    }
}

/// Convert a workflow definition from core to LSP format
fn convert_workflow(core_wf: &rhythm_core::parser::WorkflowDef) -> WorkflowDef {
    WorkflowDef {
        body: convert_stmt(&core_wf.body),
        front_matter: core_wf.front_matter.clone(),
        span: convert_span(core_wf.span),
    }
}

/// Convert a statement from core to LSP format
fn convert_stmt(stmt: &rhythm_core::executor::types::ast::Stmt) -> Stmt {
    use rhythm_core::executor::types::ast::Stmt as CoreStmt;

    match stmt {
        CoreStmt::Block { body, span } => Spanned::new(
            StmtKind::Block {
                body: body.iter().map(convert_stmt).collect(),
            },
            convert_span(*span),
        ),

        CoreStmt::Declare {
            var_kind,
            target,
            init,
            span,
        } => Spanned::new(
            StmtKind::Declare {
                var_kind: convert_var_kind(var_kind),
                target: convert_declare_target(target),
                init: init.as_ref().map(convert_expr),
            },
            convert_span(*span),
        ),

        CoreStmt::Assign {
            var,
            var_span,
            path,
            value,
            span,
        } => Spanned::new(
            StmtKind::Assign {
                var: var.clone(),
                var_span: convert_span(*var_span),
                path: path.iter().map(convert_member_access).collect(),
                value: convert_expr(value),
            },
            convert_span(*span),
        ),

        CoreStmt::If {
            test,
            then_s,
            else_s,
            span,
        } => Spanned::new(
            StmtKind::If {
                test: convert_expr(test),
                then_s: Box::new(convert_stmt(then_s)),
                else_s: else_s.as_ref().map(|s| Box::new(convert_stmt(s))),
            },
            convert_span(*span),
        ),

        CoreStmt::While { test, body, span } => Spanned::new(
            StmtKind::While {
                test: convert_expr(test),
                body: Box::new(convert_stmt(body)),
            },
            convert_span(*span),
        ),

        CoreStmt::ForLoop {
            kind,
            binding,
            binding_span,
            iterable,
            body,
            span,
        } => Spanned::new(
            StmtKind::ForLoop {
                kind: convert_for_loop_kind(kind),
                binding: binding.clone(),
                binding_span: convert_span(*binding_span),
                iterable: convert_expr(iterable),
                body: Box::new(convert_stmt(body)),
            },
            convert_span(*span),
        ),

        CoreStmt::Return { value, span } => Spanned::new(
            StmtKind::Return {
                value: value.as_ref().map(convert_expr),
            },
            convert_span(*span),
        ),

        CoreStmt::Try {
            body,
            catch_var,
            catch_var_span,
            catch_body,
            span,
        } => Spanned::new(
            StmtKind::Try {
                body: Box::new(convert_stmt(body)),
                catch_var: catch_var.clone(),
                catch_var_span: convert_span(*catch_var_span),
                catch_body: Box::new(convert_stmt(catch_body)),
            },
            convert_span(*span),
        ),

        CoreStmt::Expr { expr, span } => Spanned::new(
            StmtKind::Expr {
                expr: convert_expr(expr),
            },
            convert_span(*span),
        ),

        CoreStmt::Break { span } => Spanned::new(StmtKind::Break, convert_span(*span)),

        CoreStmt::Continue { span } => Spanned::new(StmtKind::Continue, convert_span(*span)),
    }
}

/// Convert an expression from core to LSP format
fn convert_expr(expr: &rhythm_core::executor::types::ast::Expr) -> Expr {
    use rhythm_core::executor::types::ast::Expr as CoreExpr;

    match expr {
        CoreExpr::LitBool { v, span } => {
            Spanned::new(ExprKind::LitBool { v: *v }, convert_span(*span))
        }

        CoreExpr::LitNum { v, span } => {
            Spanned::new(ExprKind::LitNum { v: *v }, convert_span(*span))
        }

        CoreExpr::LitStr { v, span } => {
            Spanned::new(ExprKind::LitStr { v: v.clone() }, convert_span(*span))
        }

        CoreExpr::LitNull { span } => Spanned::new(ExprKind::LitNull, convert_span(*span)),

        CoreExpr::LitList { elements, span } => Spanned::new(
            ExprKind::LitList {
                elements: elements.iter().map(convert_expr).collect(),
            },
            convert_span(*span),
        ),

        CoreExpr::LitObj { properties, span } => Spanned::new(
            ExprKind::LitObj {
                properties: properties
                    .iter()
                    .map(|(key, key_span, value)| {
                        (key.clone(), convert_span(*key_span), convert_expr(value))
                    })
                    .collect(),
            },
            convert_span(*span),
        ),

        CoreExpr::Ident { name, span } => {
            Spanned::new(ExprKind::Ident { name: name.clone() }, convert_span(*span))
        }

        CoreExpr::Member {
            object,
            property,
            property_span,
            optional,
            span,
        } => Spanned::new(
            ExprKind::Member {
                object: Box::new(convert_expr(object)),
                property: property.clone(),
                property_span: convert_span(*property_span),
                optional: *optional,
            },
            convert_span(*span),
        ),

        CoreExpr::Call { callee, args, span } => Spanned::new(
            ExprKind::Call {
                callee: Box::new(convert_expr(callee)),
                args: args.iter().map(convert_expr).collect(),
            },
            convert_span(*span),
        ),

        CoreExpr::Await { inner, span } => Spanned::new(
            ExprKind::Await {
                inner: Box::new(convert_expr(inner)),
            },
            convert_span(*span),
        ),

        CoreExpr::BinaryOp {
            op,
            left,
            right,
            span,
        } => Spanned::new(
            ExprKind::BinaryOp {
                op: convert_binary_op(op),
                left: Box::new(convert_expr(left)),
                right: Box::new(convert_expr(right)),
            },
            convert_span(*span),
        ),

        CoreExpr::Ternary {
            condition,
            consequent,
            alternate,
            span,
        } => Spanned::new(
            ExprKind::Ternary {
                condition: Box::new(convert_expr(condition)),
                consequent: Box::new(convert_expr(consequent)),
                alternate: Box::new(convert_expr(alternate)),
            },
            convert_span(*span),
        ),
    }
}

fn convert_var_kind(kind: &rhythm_core::executor::types::ast::VarKind) -> VarKind {
    match kind {
        rhythm_core::executor::types::ast::VarKind::Let => VarKind::Let,
        rhythm_core::executor::types::ast::VarKind::Const => VarKind::Const,
    }
}

fn convert_for_loop_kind(kind: &rhythm_core::executor::types::ast::ForLoopKind) -> ForLoopKind {
    match kind {
        rhythm_core::executor::types::ast::ForLoopKind::In => ForLoopKind::In,
        rhythm_core::executor::types::ast::ForLoopKind::Of => ForLoopKind::Of,
    }
}

fn convert_binary_op(op: &rhythm_core::executor::types::ast::BinaryOp) -> BinaryOp {
    match op {
        rhythm_core::executor::types::ast::BinaryOp::And => BinaryOp::And,
        rhythm_core::executor::types::ast::BinaryOp::Or => BinaryOp::Or,
        rhythm_core::executor::types::ast::BinaryOp::Nullish => BinaryOp::Nullish,
    }
}

fn convert_declare_target(
    target: &rhythm_core::executor::types::ast::DeclareTarget,
) -> DeclareTarget {
    use rhythm_core::executor::types::ast::DeclareTarget as CoreTarget;

    match target {
        CoreTarget::Simple { name, span } => DeclareTarget::Simple {
            name: name.clone(),
            span: convert_span(*span),
        },
        CoreTarget::Destructure { names, spans, span } => DeclareTarget::Destructure {
            names: names
                .iter()
                .zip(spans.iter())
                .map(|(name, span)| (name.clone(), convert_span(*span)))
                .collect(),
            span: convert_span(*span),
        },
    }
}

fn convert_member_access(access: &rhythm_core::executor::types::ast::MemberAccess) -> MemberAccess {
    use rhythm_core::executor::types::ast::MemberAccess as CoreAccess;

    match access {
        CoreAccess::Prop { property, span } => MemberAccess::Prop {
            property: property.clone(),
            span: convert_span(*span),
        },
        CoreAccess::Index { expr, span } => MemberAccess::Index {
            expr: convert_expr(expr),
            span: convert_span(*span),
        },
    }
}

#[cfg(test)]
mod tests;
