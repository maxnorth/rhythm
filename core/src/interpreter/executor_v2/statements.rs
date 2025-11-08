//! Statement execution handlers
//!
//! Each statement type has its own handler function that processes
//! the statement based on its current execution phase.

use super::expressions::{eval_expr, EvalResult};
use super::types::{BlockPhase, Control, Expr, ReturnPhase, Stmt, TryPhase, Val};
use super::vm::{push_stmt, Step, VM};

/* ===================== Statement Handlers ===================== */

/// Execute Block statement
pub fn execute_block(vm: &mut VM, phase: BlockPhase, idx: usize, body: Vec<Stmt>) -> Step {
    match phase {
        BlockPhase::Execute => {
            // Check if we've finished all statements in the block
            if idx >= body.len() {
                // Block complete, pop frame
                vm.frames.pop();
                return Step::Continue;
            }

            // Get the current statement to execute
            let child_stmt = &body[idx];

            // Update our frame to point to the next statement
            let frame_idx = vm.frames.len() - 1;
            vm.frames[frame_idx].kind = super::types::FrameKind::Block {
                phase: BlockPhase::Execute,
                idx: idx + 1,
            };

            // Push a frame for the child statement
            push_stmt(vm, child_stmt);

            Step::Continue
        }
    }
}

/// Execute Return statement
pub fn execute_return(vm: &mut VM, phase: ReturnPhase, value: Option<Expr>) -> Step {
    match phase {
        ReturnPhase::Eval => {
            // Evaluate the return value (if any)
            let val = if let Some(expr) = value {
                match eval_expr(&expr, &vm.env, &mut vm.resume_value) {
                    EvalResult::Value { v } => v,
                    EvalResult::Suspend { task_id } => {
                        // Expression suspended (await encountered)
                        // Set control to Suspend and stop execution
                        // DO NOT pop the frame - we need to preserve state for resumption
                        vm.control = Control::Suspend(task_id);
                        return Step::Done;
                    }
                    EvalResult::Throw { error } => {
                        // Expression threw an error
                        // Set control to Throw and DO NOT pop frame (unwinding will handle it)
                        vm.control = Control::Throw(error);
                        return Step::Continue;
                    }
                }
            } else {
                Val::Null
            };

            // Set control to Return
            vm.control = Control::Return(val);

            // Pop this frame
            vm.frames.pop();

            Step::Continue
        }
    }
}

/// Execute Try statement
pub fn execute_try(
    vm: &mut VM,
    phase: TryPhase,
    catch_var: String,
    body: Box<Stmt>,
    catch_body: Box<Stmt>,
) -> Step {
    match phase {
        TryPhase::ExecuteTry => {
            // Push the try body onto the stack
            push_stmt(vm, &body);
            Step::Continue
        }
        TryPhase::ExecuteCatch => {
            // We're now executing the catch block
            // Pop this Try frame BEFORE pushing the catch body
            vm.frames.pop();

            // Push the catch body onto the stack
            push_stmt(vm, &catch_body);

            Step::Continue
        }
    }
}
