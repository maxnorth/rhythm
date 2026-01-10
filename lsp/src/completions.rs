//! Completion provider for Rhythm language
//!
//! Provides IntelliSense completions for:
//! - Keywords (let, const, if, else, while, for, return, await, etc.)
//! - Built-in modules (Task, Timer, Signal, Workflow, Promise, Math, Inputs)
//! - Module methods (Task.run, Timer.delay, etc.)
//! - Variables in scope

use tower_lsp::lsp_types::*;

use crate::parser::ast::*;

/// All Rhythm keywords
pub const KEYWORDS: &[(&str, &str)] = &[
    ("let", "Declare a mutable variable"),
    ("const", "Declare a constant variable"),
    ("if", "Conditional statement"),
    ("else", "Else branch of conditional"),
    ("while", "While loop"),
    ("for", "For loop"),
    ("of", "Iterate over values"),
    ("in", "Iterate over keys"),
    ("return", "Return from workflow"),
    ("await", "Await a promise"),
    ("try", "Try block for error handling"),
    ("catch", "Catch block for error handling"),
    ("break", "Break out of loop"),
    ("continue", "Continue to next iteration"),
    ("true", "Boolean true"),
    ("false", "Boolean false"),
    ("null", "Null value"),
];

/// Built-in modules and their descriptions
pub const BUILTIN_MODULES: &[(&str, &str)] = &[
    ("Inputs", "Access workflow input parameters"),
    ("Task", "Execute durable tasks"),
    ("Timer", "Create delays and timers"),
    ("Signal", "Wait for external signals"),
    ("Workflow", "Execute nested workflows"),
    ("Promise", "Compose multiple promises"),
    ("Math", "Mathematical utility functions"),
];

/// Module method signatures and documentation
#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub name: &'static str,
    pub signature: &'static str,
    pub documentation: &'static str,
    pub insert_text: &'static str,
}

pub fn get_module_methods(module: &str) -> Vec<MethodInfo> {
    match module {
        "Task" => vec![MethodInfo {
            name: "run",
            signature: "Task.run(taskName: string, inputs?: object): Promise<any>",
            documentation: "Execute a durable task and return a promise for its result.\n\n\
                           The task will be executed exactly once, even if the workflow restarts.",
            insert_text: "run(\"${1:taskName}\", ${2:{}})",
        }],
        "Timer" => vec![MethodInfo {
            name: "delay",
            signature: "Timer.delay(seconds: number): Promise<void>",
            documentation: "Create a timer that resolves after the specified duration.\n\n\
                           The timer is durable - if the workflow restarts, it will resume \
                           from where it left off.",
            insert_text: "delay(${1:seconds})",
        }],
        "Signal" => vec![MethodInfo {
            name: "next",
            signature: "Signal.next(name: string): Promise<any>",
            documentation: "Wait for the next signal on the named channel.\n\n\
                           Returns the signal payload when received.",
            insert_text: "next(\"${1:signalName}\")",
        }],
        "Workflow" => vec![MethodInfo {
            name: "run",
            signature: "Workflow.run(workflowName: string, inputs?: object): Promise<any>",
            documentation: "Execute a nested workflow and return a promise for its result.\n\n\
                           Child workflows are executed durably as separate workflow instances.",
            insert_text: "run(\"${1:workflowName}\", ${2:{}})",
        }],
        "Promise" => vec![
            MethodInfo {
                name: "all",
                signature: "Promise.all(promises: Array | Object): Promise<Array | Object>",
                documentation: "Wait for all promises to resolve.\n\n\
                               Returns an array or object with all resolved values.\n\
                               Rejects if any promise rejects.",
                insert_text: "all([${1}])",
            },
            MethodInfo {
                name: "any",
                signature: "Promise.any(promises: Array | Object): Promise<{ key, value }>",
                documentation: "Wait for the first promise to resolve successfully.\n\n\
                               Returns the first resolved value with its key/index.\n\
                               Rejects only if all promises reject.",
                insert_text: "any([${1}])",
            },
            MethodInfo {
                name: "race",
                signature: "Promise.race(promises: Array | Object): Promise<{ key, value }>",
                documentation: "Wait for the first promise to settle (resolve or reject).\n\n\
                               Returns the first settled value with its key/index.",
                insert_text: "race([${1}])",
            },
        ],
        "Math" => vec![
            MethodInfo {
                name: "floor",
                signature: "Math.floor(x: number): number",
                documentation: "Round down to the nearest integer.",
                insert_text: "floor(${1:x})",
            },
            MethodInfo {
                name: "ceil",
                signature: "Math.ceil(x: number): number",
                documentation: "Round up to the nearest integer.",
                insert_text: "ceil(${1:x})",
            },
            MethodInfo {
                name: "abs",
                signature: "Math.abs(x: number): number",
                documentation: "Return the absolute value.",
                insert_text: "abs(${1:x})",
            },
            MethodInfo {
                name: "round",
                signature: "Math.round(x: number): number",
                documentation: "Round to the nearest integer.",
                insert_text: "round(${1:x})",
            },
            MethodInfo {
                name: "min",
                signature: "Math.min(a: number, b: number): number",
                documentation: "Return the smaller of two numbers.",
                insert_text: "min(${1:a}, ${2:b})",
            },
            MethodInfo {
                name: "max",
                signature: "Math.max(a: number, b: number): number",
                documentation: "Return the larger of two numbers.",
                insert_text: "max(${1:a}, ${2:b})",
            },
        ],
        _ => vec![],
    }
}

/// Array methods available on array values
pub fn get_array_methods() -> Vec<MethodInfo> {
    vec![
        MethodInfo {
            name: "length",
            signature: "array.length(): number",
            documentation: "Return the number of elements in the array.",
            insert_text: "length()",
        },
        MethodInfo {
            name: "concat",
            signature: "array.concat(other: Array): Array",
            documentation: "Return a new array with elements from both arrays.",
            insert_text: "concat(${1:[]})",
        },
        MethodInfo {
            name: "includes",
            signature: "array.includes(value: any): boolean",
            documentation: "Check if the array contains the value.",
            insert_text: "includes(${1:value})",
        },
        MethodInfo {
            name: "indexOf",
            signature: "array.indexOf(value: any): number",
            documentation: "Return the index of the value, or -1 if not found.",
            insert_text: "indexOf(${1:value})",
        },
        MethodInfo {
            name: "join",
            signature: "array.join(separator: string): string",
            documentation: "Join array elements into a string.",
            insert_text: "join(\"${1:,}\")",
        },
        MethodInfo {
            name: "slice",
            signature: "array.slice(start: number, end?: number): Array",
            documentation: "Return a portion of the array.",
            insert_text: "slice(${1:0}, ${2})",
        },
        MethodInfo {
            name: "reverse",
            signature: "array.reverse(): Array",
            documentation: "Return a new array with elements in reverse order.",
            insert_text: "reverse()",
        },
    ]
}

/// String methods available on string values
pub fn get_string_methods() -> Vec<MethodInfo> {
    vec![
        MethodInfo {
            name: "length",
            signature: "string.length(): number",
            documentation: "Return the length of the string.",
            insert_text: "length()",
        },
        MethodInfo {
            name: "toUpperCase",
            signature: "string.toUpperCase(): string",
            documentation: "Convert to uppercase.",
            insert_text: "toUpperCase()",
        },
        MethodInfo {
            name: "toLowerCase",
            signature: "string.toLowerCase(): string",
            documentation: "Convert to lowercase.",
            insert_text: "toLowerCase()",
        },
        MethodInfo {
            name: "trim",
            signature: "string.trim(): string",
            documentation: "Remove whitespace from both ends.",
            insert_text: "trim()",
        },
        MethodInfo {
            name: "split",
            signature: "string.split(separator: string): Array",
            documentation: "Split string into array.",
            insert_text: "split(\"${1}\")",
        },
        MethodInfo {
            name: "startsWith",
            signature: "string.startsWith(prefix: string): boolean",
            documentation: "Check if string starts with prefix.",
            insert_text: "startsWith(\"${1}\")",
        },
        MethodInfo {
            name: "endsWith",
            signature: "string.endsWith(suffix: string): boolean",
            documentation: "Check if string ends with suffix.",
            insert_text: "endsWith(\"${1}\")",
        },
        MethodInfo {
            name: "includes",
            signature: "string.includes(substring: string): boolean",
            documentation: "Check if string contains substring.",
            insert_text: "includes(\"${1}\")",
        },
        MethodInfo {
            name: "replace",
            signature: "string.replace(search: string, replacement: string): string",
            documentation: "Replace first occurrence.",
            insert_text: "replace(\"${1:search}\", \"${2:replacement}\")",
        },
        MethodInfo {
            name: "substring",
            signature: "string.substring(start: number, end?: number): string",
            documentation: "Extract a portion of the string.",
            insert_text: "substring(${1:0}, ${2})",
        },
    ]
}

/// Context for completion
#[derive(Debug, Clone)]
pub struct CompletionContext {
    /// The character that triggered completion (if any)
    #[allow(dead_code)]
    pub trigger_char: Option<char>,
    /// Text before the cursor on the current line
    #[allow(dead_code)]
    pub prefix: String,
    /// Variables currently in scope
    pub variables: Vec<String>,
    /// Whether we're after a dot (member access)
    pub after_dot: bool,
    /// The identifier before the dot (if after_dot is true)
    pub dot_target: Option<String>,
}

impl CompletionContext {
    pub fn from_position(source: &str, line: u32, character: u32) -> Self {
        let lines: Vec<&str> = source.lines().collect();
        let current_line = lines.get(line as usize).unwrap_or(&"");

        let prefix = if (character as usize) <= current_line.len() {
            current_line[..character as usize].to_string()
        } else {
            current_line.to_string()
        };

        // Check if we're after a dot
        let trimmed = prefix.trim_end();
        let after_dot = trimmed.ends_with('.');

        // Get the identifier before the dot
        let dot_target = if after_dot {
            // Remove trailing dot and get the last identifier
            let before_dot = &trimmed[..trimmed.len() - 1];
            before_dot
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .last()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
        } else {
            None
        };

        // Collect variables from the source (simplified - just looks for let/const declarations)
        let mut variables = Vec::new();
        for line in lines.iter().take(line as usize + 1) {
            let trimmed = line.trim();
            if trimmed.starts_with("let ") || trimmed.starts_with("const ") {
                if let Some(name) = trimmed
                    .split_whitespace()
                    .nth(1)
                    .map(|s| s.trim_end_matches('=').trim())
                {
                    if !name.starts_with('{') {
                        variables.push(name.to_string());
                    }
                }
            }
        }

        CompletionContext {
            trigger_char: prefix.chars().last(),
            prefix,
            variables,
            after_dot,
            dot_target,
        }
    }
}

/// Get completions for the given context
pub fn get_completions(ctx: &CompletionContext) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    if ctx.after_dot {
        // Member access completions
        if let Some(target) = &ctx.dot_target {
            // Check if it's a builtin module
            let methods = get_module_methods(target);
            if !methods.is_empty() {
                for method in methods {
                    items.push(CompletionItem {
                        label: method.name.to_string(),
                        kind: Some(CompletionItemKind::METHOD),
                        detail: Some(method.signature.to_string()),
                        documentation: Some(Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: method.documentation.to_string(),
                        })),
                        insert_text: Some(method.insert_text.to_string()),
                        insert_text_format: Some(InsertTextFormat::SNIPPET),
                        ..Default::default()
                    });
                }
            } else {
                // Could be an array or string - provide both sets of methods
                for method in get_array_methods() {
                    items.push(CompletionItem {
                        label: method.name.to_string(),
                        kind: Some(CompletionItemKind::METHOD),
                        detail: Some(method.signature.to_string()),
                        documentation: Some(Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: method.documentation.to_string(),
                        })),
                        insert_text: Some(method.insert_text.to_string()),
                        insert_text_format: Some(InsertTextFormat::SNIPPET),
                        ..Default::default()
                    });
                }
                for method in get_string_methods() {
                    items.push(CompletionItem {
                        label: method.name.to_string(),
                        kind: Some(CompletionItemKind::METHOD),
                        detail: Some(method.signature.to_string()),
                        documentation: Some(Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: method.documentation.to_string(),
                        })),
                        insert_text: Some(method.insert_text.to_string()),
                        insert_text_format: Some(InsertTextFormat::SNIPPET),
                        ..Default::default()
                    });
                }
            }
        }
    } else {
        // Top-level completions

        // Keywords
        for (keyword, description) in KEYWORDS {
            items.push(CompletionItem {
                label: keyword.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some(description.to_string()),
                ..Default::default()
            });
        }

        // Builtin modules
        for (module, description) in BUILTIN_MODULES {
            items.push(CompletionItem {
                label: module.to_string(),
                kind: Some(CompletionItemKind::MODULE),
                detail: Some(description.to_string()),
                ..Default::default()
            });
        }

        // Variables in scope
        for var in &ctx.variables {
            items.push(CompletionItem {
                label: var.clone(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some("Variable".to_string()),
                ..Default::default()
            });
        }
    }

    items
}

/// Get signature help for a function call
pub fn get_signature_help(
    source: &str,
    line: u32,
    character: u32,
) -> Option<SignatureHelp> {
    let lines: Vec<&str> = source.lines().collect();
    let current_line = lines.get(line as usize)?;

    let prefix = if (character as usize) <= current_line.len() {
        &current_line[..character as usize]
    } else {
        current_line
    };

    // Find the most recent unclosed parenthesis and the function name before it
    let mut paren_depth = 0;
    let mut func_end = None;

    for (i, c) in prefix.char_indices().rev() {
        match c {
            ')' => paren_depth += 1,
            '(' => {
                if paren_depth == 0 {
                    func_end = Some(i);
                    break;
                }
                paren_depth -= 1;
            }
            _ => {}
        }
    }

    let func_end = func_end?;
    let before_paren = &prefix[..func_end];

    // Extract the function name (could be Module.method or just method)
    let parts: Vec<&str> = before_paren
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '.')
        .filter(|s| !s.is_empty())
        .collect();

    let func_path = parts.last()?;

    // Check if it's a module method call (e.g., "Task.run")
    if let Some(dot_pos) = func_path.rfind('.') {
        let module = &func_path[..dot_pos];
        let method = &func_path[dot_pos + 1..];

        let methods = get_module_methods(module);
        for m in methods {
            if m.name == method {
                return Some(SignatureHelp {
                    signatures: vec![SignatureInformation {
                        label: m.signature.to_string(),
                        documentation: Some(Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: m.documentation.to_string(),
                        })),
                        parameters: None,
                        active_parameter: None,
                    }],
                    active_signature: Some(0),
                    active_parameter: None,
                });
            }
        }
    }

    None
}

/// Collect all variable declarations from the AST
pub fn collect_variables(stmt: &Stmt) -> Vec<(String, Span)> {
    let mut vars = Vec::new();
    collect_variables_from_stmt(stmt, &mut vars);
    vars
}

fn collect_variables_from_stmt(stmt: &Stmt, vars: &mut Vec<(String, Span)>) {
    match &stmt.node {
        StmtKind::Block { body } => {
            for s in body {
                collect_variables_from_stmt(s, vars);
            }
        }
        StmtKind::Declare { target, .. } => match target {
            DeclareTarget::Simple { name, span } => {
                vars.push((name.clone(), *span));
            }
            DeclareTarget::Destructure { names, .. } => {
                for (name, span) in names {
                    vars.push((name.clone(), *span));
                }
            }
        },
        StmtKind::ForLoop {
            binding,
            binding_span,
            body,
            ..
        } => {
            vars.push((binding.clone(), *binding_span));
            collect_variables_from_stmt(body, vars);
        }
        StmtKind::Try {
            body,
            catch_var,
            catch_var_span,
            catch_body,
        } => {
            collect_variables_from_stmt(body, vars);
            vars.push((catch_var.clone(), *catch_var_span));
            collect_variables_from_stmt(catch_body, vars);
        }
        StmtKind::If {
            then_s, else_s, ..
        } => {
            collect_variables_from_stmt(then_s, vars);
            if let Some(else_s) = else_s {
                collect_variables_from_stmt(else_s, vars);
            }
        }
        StmtKind::While { body, .. } => {
            collect_variables_from_stmt(body, vars);
        }
        _ => {}
    }
}
