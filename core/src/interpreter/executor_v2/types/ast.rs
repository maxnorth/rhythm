//! Abstract Syntax Tree node types

use serde::{Deserialize, Serialize};

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
    Let {
        name: String,
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
    For {
        iterator: String,
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

/// Expression AST node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum Expr {
    LitBool { v: bool },
    LitNum { v: f64 },
    LitStr { v: String },
    LitNull,
    LitList { elements: Vec<Expr> },
    LitObj { properties: Vec<(String, Expr)> },
    Ident { name: String },
    Member { object: Box<Expr>, property: String },
    Call { callee: Box<Expr>, args: Vec<Expr> },
    Await { inner: Box<Expr> },
}
