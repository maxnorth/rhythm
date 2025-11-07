//! Abstract Syntax Tree node types

use serde::{Deserialize, Serialize};

/// Statement AST node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Stmt {
    Block {
        body: Vec<Stmt>,
    },
    Let {
        name: String,
        init: Option<Expr>,
    },
    Assign {
        name: String,
        expr: Expr,
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
    Break,
    Continue,
}

/// Expression AST node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Expr {
    LitBool { v: bool },
    LitNum { v: f64 },
    LitStr { v: String },
    Ident { name: String },
    Member { object: Box<Expr>, property: String },
    Call { callee: Box<Expr>, args: Vec<Expr> },
    Await { inner: Box<Expr> },
}
