//! Abstract Syntax Tree node types

use serde::{Deserialize, Serialize};

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
    Simple { name: String },
    Destructure { names: Vec<String> },
}

/// Member access segment for assignment paths
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum MemberAccess {
    Prop { property: String },
    Index { expr: Expr },
}

/// Statement AST node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum Stmt {
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
        iterable: Expr,
        body: Box<Stmt>,
    },
    Return {
        value: Option<Expr>,
    },
    Try {
        body: Box<Stmt>,
        catch_var: String,
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

/// Expression AST node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum Expr {
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
        properties: Vec<(String, Expr)>,
    },
    Ident {
        name: String,
    },
    Member {
        object: Box<Expr>,
        property: String,
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
