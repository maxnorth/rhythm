//! Abstract Syntax Tree node types

use serde::{Deserialize, Serialize};

/// Source location span for error reporting and LSP features
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Span {
    /// Start byte offset
    pub start: usize,
    /// End byte offset
    pub end: usize,
    /// Start line (0-indexed)
    pub start_line: usize,
    /// Start column (0-indexed)
    pub start_col: usize,
    /// End line (0-indexed)
    pub end_line: usize,
    /// End column (0-indexed)
    pub end_col: usize,
}

impl Span {
    pub fn new(
        start: usize,
        end: usize,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> Self {
        Self {
            start,
            end,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Create a span that covers both self and other
    pub fn merge(&self, other: &Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            start_line: if self.start <= other.start {
                self.start_line
            } else {
                other.start_line
            },
            start_col: if self.start <= other.start {
                self.start_col
            } else {
                other.start_col
            },
            end_line: if self.end >= other.end {
                self.end_line
            } else {
                other.end_line
            },
            end_col: if self.end >= other.end {
                self.end_col
            } else {
                other.end_col
            },
        }
    }
}

/// Variable declaration kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VarKind {
    Let,
    Const,
}

/// For loop kind (in vs of)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForLoopKind {
    /// for (let k in obj) - iterates over keys
    In,
    /// for (let v of arr) - iterates over values
    Of,
}

/// Target for variable declaration (simple identifier or destructure pattern)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum DeclareTarget {
    Simple {
        name: String,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    Destructure {
        names: Vec<String>,
        /// Spans for each individual name (parallel to names)
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        spans: Vec<Span>,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
}

/// Member access segment for assignment paths
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum MemberAccess {
    Prop {
        property: String,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    Index {
        expr: Expr,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
}

/// Statement AST node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum Stmt {
    Block {
        body: Vec<Stmt>,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    Declare {
        var_kind: VarKind,
        target: DeclareTarget,
        init: Option<Expr>,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    Assign {
        var: String,
        #[serde(default, skip_serializing_if = "is_default_span")]
        var_span: Span,
        path: Vec<MemberAccess>,
        value: Expr,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    If {
        test: Expr,
        then_s: Box<Stmt>,
        else_s: Option<Box<Stmt>>,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    While {
        test: Expr,
        body: Box<Stmt>,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    ForLoop {
        kind: ForLoopKind,
        binding: String,
        #[serde(default, skip_serializing_if = "is_default_span")]
        binding_span: Span,
        iterable: Expr,
        body: Box<Stmt>,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    Return {
        value: Option<Expr>,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    Try {
        body: Box<Stmt>,
        catch_var: String,
        #[serde(default, skip_serializing_if = "is_default_span")]
        catch_var_span: Span,
        catch_body: Box<Stmt>,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    Expr {
        expr: Expr,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    Break {
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    Continue {
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
}

impl Stmt {
    /// Get the span of this statement
    pub fn span(&self) -> Span {
        match self {
            Stmt::Block { span, .. } => *span,
            Stmt::Declare { span, .. } => *span,
            Stmt::Assign { span, .. } => *span,
            Stmt::If { span, .. } => *span,
            Stmt::While { span, .. } => *span,
            Stmt::ForLoop { span, .. } => *span,
            Stmt::Return { span, .. } => *span,
            Stmt::Try { span, .. } => *span,
            Stmt::Expr { span, .. } => *span,
            Stmt::Break { span } => *span,
            Stmt::Continue { span } => *span,
        }
    }
}

/// Binary operator for short-circuit evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BinaryOp {
    And,     // &&
    Or,      // ||
    Nullish, // ??
}

/// Expression AST node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum Expr {
    LitBool {
        v: bool,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    LitNum {
        v: f64,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    LitStr {
        v: String,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    LitNull {
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    LitList {
        elements: Vec<Expr>,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    LitObj {
        /// Properties as (key, key_span, value) tuples
        properties: Vec<(String, Span, Expr)>,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    Ident {
        name: String,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    Member {
        object: Box<Expr>,
        property: String,
        #[serde(default, skip_serializing_if = "is_default_span")]
        property_span: Span,
        optional: bool,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    Await {
        inner: Box<Expr>,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    BinaryOp {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
    Ternary {
        condition: Box<Expr>,
        consequent: Box<Expr>,
        alternate: Box<Expr>,
        #[serde(default, skip_serializing_if = "is_default_span")]
        span: Span,
    },
}

impl Expr {
    /// Get the span of this expression
    pub fn span(&self) -> Span {
        match self {
            Expr::LitBool { span, .. } => *span,
            Expr::LitNum { span, .. } => *span,
            Expr::LitStr { span, .. } => *span,
            Expr::LitNull { span } => *span,
            Expr::LitList { span, .. } => *span,
            Expr::LitObj { span, .. } => *span,
            Expr::Ident { span, .. } => *span,
            Expr::Member { span, .. } => *span,
            Expr::Call { span, .. } => *span,
            Expr::Await { span, .. } => *span,
            Expr::BinaryOp { span, .. } => *span,
            Expr::Ternary { span, .. } => *span,
        }
    }
}

/// Helper function for serde to skip serializing default spans
fn is_default_span(span: &Span) -> bool {
    *span == Span::default()
}
