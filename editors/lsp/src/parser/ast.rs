//! Abstract Syntax Tree with span information for LSP
//!
//! This is a modified version of rhythm-core's AST that includes source locations
//! for diagnostics, hover, and go-to-definition support.

use serde::{Deserialize, Serialize};

/// Source location span
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Span {
    /// Start offset (byte index)
    pub start: usize,
    /// End offset (byte index)
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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

/// A value with an associated span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
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
pub enum DeclareTarget {
    Simple {
        name: String,
        span: Span,
    },
    Destructure {
        names: Vec<(String, Span)>,
        span: Span,
    },
}

impl DeclareTarget {
    #[allow(dead_code)]
    pub fn span(&self) -> Span {
        match self {
            DeclareTarget::Simple { span, .. } => *span,
            DeclareTarget::Destructure { span, .. } => *span,
        }
    }
}

/// Member access segment for assignment paths
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemberAccess {
    Prop { property: String, span: Span },
    Index { expr: Expr, span: Span },
}

/// Statement AST node with span
pub type Stmt = Spanned<StmtKind>;

/// Statement kinds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StmtKind {
    Block {
        body: Vec<Stmt>,
    },
    Declare {
        var_kind: VarKind,
        target: DeclareTarget,
        init: Option<Expr>,
    },
    Assign {
        var: String,
        var_span: Span,
        path: Vec<MemberAccess>,
        value: Expr,
    },
    If {
        test: Expr,
        then_s: Box<Stmt>,
        else_s: Option<Box<Stmt>>,
    },
    While {
        test: Expr,
        body: Box<Stmt>,
    },
    ForLoop {
        kind: ForLoopKind,
        binding: String,
        binding_span: Span,
        iterable: Expr,
        body: Box<Stmt>,
    },
    Return {
        value: Option<Expr>,
    },
    Try {
        body: Box<Stmt>,
        catch_var: String,
        catch_var_span: Span,
        catch_body: Box<Stmt>,
    },
    Expr {
        expr: Expr,
    },
    Break,
    Continue,
}

/// Binary operator for short-circuit evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BinaryOp {
    And,     // &&
    Or,      // ||
    Nullish, // ??
}

/// Expression AST node with span
pub type Expr = Spanned<ExprKind>;

/// Expression kinds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExprKind {
    LitBool {
        v: bool,
    },
    LitNum {
        v: f64,
    },
    LitStr {
        v: String,
    },
    LitNull,
    LitList {
        elements: Vec<Expr>,
    },
    LitObj {
        properties: Vec<(String, Span, Expr)>,
    },
    Ident {
        name: String,
    },
    Member {
        object: Box<Expr>,
        property: String,
        property_span: Span,
        optional: bool,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Await {
        inner: Box<Expr>,
    },
    BinaryOp {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Ternary {
        condition: Box<Expr>,
        consequent: Box<Expr>,
        alternate: Box<Expr>,
    },
}

/// Workflow definition with span information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDef {
    /// Workflow body (statements to execute)
    pub body: Stmt,
    /// Optional YAML front matter
    pub front_matter: Option<String>,
    /// Span of the entire workflow
    pub span: Span,
}

/// Symbol information for go-to-definition and references
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub span: Span,
    pub definition_span: Option<Span>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Variable,
    Parameter,
    Property,
    BuiltinModule,
    BuiltinMethod,
}
