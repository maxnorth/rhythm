//! Core execution loop
//!
//! This module contains the step() function - the heart of the interpreter.
//! It processes one frame at a time, advancing PCs and managing the frame stack.

use super::types::{Control, Expr, FrameKind, ReturnPc, Stmt, Val};
use super::vm::{Step, VM};

/* ===================== Expression Evaluation ===================== */

/// Evaluate an expression to a value
///
/// Milestone 1: Only supports literals
fn eval_expr(expr: &Expr) -> Result<Val, String> {
    match expr {
        Expr::LitBool { v } => Ok(Val::Bool(*v)),
        Expr::LitNum { v } => Ok(Val::Num(*v)),
        Expr::LitStr { v } => Ok(Val::Str(v.clone())),

        // Not yet implemented
        Expr::Ident { .. } => Err("Identifiers not yet supported".to_string()),
        Expr::Member { .. } => Err("Member expressions not yet supported".to_string()),
        Expr::Call { .. } => Err("Call expressions not yet supported".to_string()),
        Expr::Await { .. } => Err("Await expressions not yet supported".to_string()),
    }
}

/* ===================== Core Loop ===================== */

/// Execute one step of the VM
///
/// This is the core interpreter loop. It:
/// 1. Checks for active control flow (not needed in Milestone 1)
/// 2. Gets the top frame
/// 3. Matches on frame kind and PC
/// 4. Executes the appropriate logic
/// 5. Either continues or signals done
pub fn step(vm: &mut VM) -> Step {
    // Get top frame (if any)
    let Some(frame_idx) = vm.frames.len().checked_sub(1) else {
        // No frames left - execution complete
        return Step::Done;
    };

    // Clone frame data we need (to avoid borrow checker issues)
    let (kind, node) = {
        let f = &vm.frames[frame_idx];
        (f.kind.clone(), f.node.clone())
    };

    // Match on frame kind and execute
    match (kind, node) {
        // Return statement
        (FrameKind::Return { pc }, Stmt::Return { value }) => match pc {
            ReturnPc::Eval => {
                // Evaluate the return value (if any)
                let val = if let Some(expr) = value {
                    match eval_expr(&expr) {
                        Ok(v) => Some(v),
                        Err(e) => {
                            // For now, panic on eval errors
                            // Later we'll convert to Control::Throw
                            panic!("Expression evaluation failed: {}", e);
                        }
                    }
                } else {
                    None
                };

                // Set control to Return
                vm.control = Control::Return(val);

                // Pop this frame
                vm.frames.pop();

                Step::Continue
            }

            ReturnPc::Done => {
                // Should never reach here
                vm.frames.pop();
                Step::Continue
            }
        },

        // Shouldn't happen - frame kind doesn't match node
        _ => panic!("Frame kind does not match statement node"),
    }
}

/// Run the VM until it completes
///
/// This is a helper that calls step() in a loop until done.
/// Returns the final control state (which should be Return for normal completion).
pub fn run_until_done(vm: &mut VM) -> Control {
    loop {
        match step(vm) {
            Step::Continue => continue,
            Step::Done => break,
        }
    }

    vm.control.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::executor_v2::types::Stmt;
    use crate::interpreter::executor_v2::vm::VM;

    #[test]
    fn test_return_literal_num() {
        let program = Stmt::Return {
            value: Some(Expr::LitNum { v: 42.0 }),
        };

        let mut vm = VM::new(program);
        let result = run_until_done(&mut vm);

        assert_eq!(result, Control::Return(Some(Val::Num(42.0))));
    }

    #[test]
    fn test_return_literal_bool() {
        let program = Stmt::Return {
            value: Some(Expr::LitBool { v: true }),
        };

        let mut vm = VM::new(program);
        let result = run_until_done(&mut vm);

        assert_eq!(result, Control::Return(Some(Val::Bool(true))));
    }

    #[test]
    fn test_return_literal_str() {
        let program = Stmt::Return {
            value: Some(Expr::LitStr {
                v: "hello".to_string(),
            }),
        };

        let mut vm = VM::new(program);
        let result = run_until_done(&mut vm);

        assert_eq!(
            result,
            Control::Return(Some(Val::Str("hello".to_string())))
        );
    }

    #[test]
    fn test_return_unit() {
        let program = Stmt::Return { value: None };

        let mut vm = VM::new(program);
        let result = run_until_done(&mut vm);

        assert_eq!(result, Control::Return(None));
    }
}
