//! Hover information provider for Rhythm language
//!
//! Provides documentation and type information on hover.

use tower_lsp::lsp_types::*;

use crate::completions::{
    get_array_methods, get_module_methods, get_string_methods, BUILTIN_MODULES, KEYWORDS,
};
use crate::parser::ast::*;

/// Get hover information for a position in the source
pub fn get_hover(source: &str, line: u32, character: u32) -> Option<Hover> {
    let word = get_word_at_position(source, line, character)?;

    // Check if it's a keyword
    for (keyword, description) in KEYWORDS {
        if word == *keyword {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("**{}** (keyword)\n\n{}", keyword, description),
                }),
                range: None,
            });
        }
    }

    // Check if it's a builtin module
    for (module, description) in BUILTIN_MODULES {
        if word == *module {
            let methods = get_module_methods(module);
            let methods_doc = if methods.is_empty() {
                String::new()
            } else {
                let method_list: Vec<String> = methods
                    .iter()
                    .map(|m| format!("- `{}`", m.signature))
                    .collect();
                format!("\n\n**Methods:**\n{}", method_list.join("\n"))
            };

            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("**{}** (module)\n\n{}{}", module, description, methods_doc),
                }),
                range: None,
            });
        }
    }

    // Check if it's a method of a builtin module
    // We need to look at the context to see if there's a dot before
    let context = get_hover_context(source, line, character);
    if let Some(module) = context.module {
        let methods = get_module_methods(&module);
        for method in methods {
            if word == method.name {
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!(
                            "```rhythm\n{}\n```\n\n{}",
                            method.signature, method.documentation
                        ),
                    }),
                    range: None,
                });
            }
        }
    }

    // Check array/string methods
    for method in get_array_methods() {
        if word == method.name {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        "```rhythm\n{}\n```\n\n{}",
                        method.signature, method.documentation
                    ),
                }),
                range: None,
            });
        }
    }

    for method in get_string_methods() {
        if word == method.name {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        "```rhythm\n{}\n```\n\n{}",
                        method.signature, method.documentation
                    ),
                }),
                range: None,
            });
        }
    }

    None
}

/// Context for hover lookup
struct HoverContext {
    /// The module being accessed (if any)
    module: Option<String>,
}

fn get_hover_context(source: &str, line: u32, character: u32) -> HoverContext {
    let lines: Vec<&str> = source.lines().collect();
    let current_line = lines.get(line as usize).unwrap_or(&"");

    // Look backwards from cursor for a dot and preceding identifier
    let prefix = if (character as usize) <= current_line.len() {
        &current_line[..character as usize]
    } else {
        current_line
    };

    // Find the start of the current word
    let word_start = prefix
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);

    // Check if there's a dot before the word
    if word_start > 0 {
        let before_word = &prefix[..word_start];
        if let Some(before_dot) = before_word.strip_suffix('.') {
            // Get the identifier before the dot
            let module_start = before_dot
                .rfind(|c: char| !c.is_alphanumeric() && c != '_')
                .map(|i| i + 1)
                .unwrap_or(0);
            let module = &before_dot[module_start..];
            if !module.is_empty() {
                return HoverContext {
                    module: Some(module.to_string()),
                };
            }
        }
    }

    HoverContext { module: None }
}

/// Get the word at the given position
fn get_word_at_position(source: &str, line: u32, character: u32) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let current_line = lines.get(line as usize)?;

    let char_idx = character as usize;
    if char_idx > current_line.len() {
        return None;
    }

    // Find word boundaries
    let before = &current_line[..char_idx];
    let after = &current_line[char_idx..];

    let start = before
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);

    let end = after
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(after.len());

    let word = format!("{}{}", &before[start..], &after[..end]);

    if word.is_empty() {
        None
    } else {
        Some(word)
    }
}

/// Get documentation for a specific expression at a position (using AST)
pub fn get_hover_from_ast(
    workflow: &WorkflowDef,
    source: &str,
    line: u32,
    character: u32,
) -> Option<Hover> {
    // First try the simple text-based approach
    if let Some(hover) = get_hover(source, line, character) {
        return Some(hover);
    }

    // Then try to find the expression in the AST
    let offset = line_col_to_offset(source, line as usize, character as usize);

    // Find the expression at this offset
    if let Some(expr) = find_expr_at_offset(&workflow.body, offset) {
        return hover_for_expr(&expr);
    }

    None
}

fn line_col_to_offset(source: &str, line: usize, col: usize) -> usize {
    let mut offset = 0;
    for (i, l) in source.lines().enumerate() {
        if i == line {
            return offset + col.min(l.len());
        }
        offset += l.len() + 1; // +1 for newline
    }
    offset
}

fn find_expr_at_offset(stmt: &Stmt, offset: usize) -> Option<Expr> {
    match &stmt.node {
        StmtKind::Block { body } => {
            for s in body {
                if let Some(e) = find_expr_at_offset(s, offset) {
                    return Some(e);
                }
            }
        }
        StmtKind::Declare {
            init: Some(init), ..
        } => {
            if let Some(e) = find_expr_at_offset_in_expr(init, offset) {
                return Some(e);
            }
        }
        StmtKind::Assign { value, .. } => {
            if let Some(e) = find_expr_at_offset_in_expr(value, offset) {
                return Some(e);
            }
        }
        StmtKind::If {
            test,
            then_s,
            else_s,
        } => {
            if let Some(e) = find_expr_at_offset_in_expr(test, offset) {
                return Some(e);
            }
            if let Some(e) = find_expr_at_offset(then_s, offset) {
                return Some(e);
            }
            if let Some(else_s) = else_s {
                if let Some(e) = find_expr_at_offset(else_s, offset) {
                    return Some(e);
                }
            }
        }
        StmtKind::While { test, body } => {
            if let Some(e) = find_expr_at_offset_in_expr(test, offset) {
                return Some(e);
            }
            if let Some(e) = find_expr_at_offset(body, offset) {
                return Some(e);
            }
        }
        StmtKind::ForLoop { iterable, body, .. } => {
            if let Some(e) = find_expr_at_offset_in_expr(iterable, offset) {
                return Some(e);
            }
            if let Some(e) = find_expr_at_offset(body, offset) {
                return Some(e);
            }
        }
        StmtKind::Return { value: Some(value) } => {
            if let Some(e) = find_expr_at_offset_in_expr(value, offset) {
                return Some(e);
            }
        }
        StmtKind::Try {
            body, catch_body, ..
        } => {
            if let Some(e) = find_expr_at_offset(body, offset) {
                return Some(e);
            }
            if let Some(e) = find_expr_at_offset(catch_body, offset) {
                return Some(e);
            }
        }
        StmtKind::Expr { expr } => {
            if let Some(e) = find_expr_at_offset_in_expr(expr, offset) {
                return Some(e);
            }
        }
        _ => {}
    }
    None
}

fn find_expr_at_offset_in_expr(expr: &Expr, offset: usize) -> Option<Expr> {
    // Check if offset is within this expression's span
    if offset >= expr.span.start && offset <= expr.span.end {
        // Try to find a more specific child expression
        match &expr.node {
            ExprKind::Member { object, .. } => {
                if let Some(e) = find_expr_at_offset_in_expr(object, offset) {
                    return Some(e);
                }
            }
            ExprKind::Call { callee, args } => {
                if let Some(e) = find_expr_at_offset_in_expr(callee, offset) {
                    return Some(e);
                }
                for arg in args {
                    if let Some(e) = find_expr_at_offset_in_expr(arg, offset) {
                        return Some(e);
                    }
                }
            }
            ExprKind::Await { inner } => {
                if let Some(e) = find_expr_at_offset_in_expr(inner, offset) {
                    return Some(e);
                }
            }
            ExprKind::BinaryOp { left, right, .. } => {
                if let Some(e) = find_expr_at_offset_in_expr(left, offset) {
                    return Some(e);
                }
                if let Some(e) = find_expr_at_offset_in_expr(right, offset) {
                    return Some(e);
                }
            }
            ExprKind::Ternary {
                condition,
                consequent,
                alternate,
            } => {
                if let Some(e) = find_expr_at_offset_in_expr(condition, offset) {
                    return Some(e);
                }
                if let Some(e) = find_expr_at_offset_in_expr(consequent, offset) {
                    return Some(e);
                }
                if let Some(e) = find_expr_at_offset_in_expr(alternate, offset) {
                    return Some(e);
                }
            }
            ExprKind::LitList { elements } => {
                for elem in elements {
                    if let Some(e) = find_expr_at_offset_in_expr(elem, offset) {
                        return Some(e);
                    }
                }
            }
            ExprKind::LitObj { properties } => {
                for (_, _, value) in properties {
                    if let Some(e) = find_expr_at_offset_in_expr(value, offset) {
                        return Some(e);
                    }
                }
            }
            _ => {}
        }
        // Return this expression if no child matches
        return Some(expr.clone());
    }
    None
}

fn hover_for_expr(expr: &Expr) -> Option<Hover> {
    match &expr.node {
        ExprKind::Ident { name } => {
            // Check if it's a builtin
            for (module, description) in BUILTIN_MODULES {
                if name == *module {
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!("**{}** (module)\n\n{}", module, description),
                        }),
                        range: None,
                    });
                }
            }
            // It's a variable
            Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("**{}** (variable)", name),
                }),
                range: None,
            })
        }
        ExprKind::LitBool { v } => Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("**boolean**: `{}`", v),
            }),
            range: None,
        }),
        ExprKind::LitNum { v } => Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("**number**: `{}`", v),
            }),
            range: None,
        }),
        ExprKind::LitStr { v } => Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("**string**: `\"{}\"`", v),
            }),
            range: None,
        }),
        ExprKind::LitNull => Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: "**null**".to_string(),
            }),
            range: None,
        }),
        _ => None,
    }
}
