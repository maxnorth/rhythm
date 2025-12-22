//! Core execution loop
//!
//! This module contains the step() function - the heart of the interpreter.
//! It processes one frame at a time, advancing execution phases and managing the frame stack.
//!
//! ## Function Organization
//! Functions are ordered by importance/call hierarchy:
//! 1. run_until_done() - Top-level driver (calls step repeatedly)
//! 2. step() - Main execution loop (dispatches to statement handlers)

use super::statements::{
    execute_assign, execute_block, execute_break, execute_continue, execute_declare, execute_expr,
    execute_if, execute_return, execute_try, execute_while,
};
use super::types::{Control, FrameKind, Stmt};
use super::vm::{Step, VM};

/* ===================== Public API ===================== */

/// Run the VM until it completes
///
/// This is the top-level driver that repeatedly calls step() until execution finishes.
/// After completion, inspect `vm.control` for the final state and `vm.outbox` for side effects.
pub fn run_until_done(vm: &mut VM) {
    while let Step::Continue = step(vm) {}
}

/// Execute one step of the VM
///
/// This is the core interpreter loop. It:
/// 1. Checks for active control flow and unwinds if needed
/// 2. Gets the top frame
/// 3. Matches on frame kind and execution phase
/// 4. Executes the appropriate logic
/// 5. Either continues or signals done
pub fn step(vm: &mut VM) -> Step {
    // Check if we have active control flow (return/break/continue/throw)
    if vm.control != Control::None {
        // Unwind: pop frames until we find a handler or run out of frames
        return unwind(vm);
    }

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

    // Dispatch to statement handler
    match (kind, node) {
        (FrameKind::Return { phase }, Stmt::Return { value }) => execute_return(vm, phase, value),

        (
            FrameKind::Block {
                phase,
                idx,
                declared_vars,
            },
            Stmt::Block { body },
        ) => {
            // Clone once to get ownership
            let declared_vars = declared_vars.clone();
            execute_block(vm, phase, idx, declared_vars, body.as_slice())
        }

        (
            FrameKind::Try { phase, catch_var },
            Stmt::Try {
                body,
                catch_var: _,
                catch_body,
            },
        ) => execute_try(vm, phase, catch_var, body, catch_body),

        (FrameKind::Expr { phase }, Stmt::Expr { expr }) => execute_expr(vm, phase, expr),

        (FrameKind::Assign { phase }, Stmt::Assign { var, path, value }) => {
            execute_assign(vm, phase, var, path, value)
        }

        (
            FrameKind::If { phase },
            Stmt::If {
                test,
                then_s,
                else_s,
            },
        ) => execute_if(vm, phase, test, then_s, else_s),

        (FrameKind::While { phase, label: _ }, Stmt::While { test, body }) => {
            execute_while(vm, phase, test, body)
        }

        (FrameKind::Break { phase }, Stmt::Break) => execute_break(vm, phase),

        (FrameKind::Continue { phase }, Stmt::Continue) => execute_continue(vm, phase),

        (
            FrameKind::Declare { phase },
            Stmt::Declare {
                var_kind,
                name,
                init,
            },
        ) => execute_declare(vm, phase, var_kind, name, init),

        // Shouldn't happen - frame kind doesn't match node
        _ => panic!("Frame kind does not match statement node"),
    }
}

/* ===================== Control Flow ===================== */

/// Unwind the stack when control flow is active
///
/// Pops frames until we find an appropriate handler or run out of frames.
/// For Suspend, we DO NOT unwind - we preserve the stack for resumption.
fn unwind(vm: &mut VM) -> Step {
    match &vm.control {
        Control::Return(_) => {
            // Pop all frames, cleaning up block scopes as we go
            while let Some(frame) = vm.frames.pop() {
                // Clean up any variables declared in this block
                if let FrameKind::Block { declared_vars, .. } = frame.kind {
                    for var_name in declared_vars {
                        vm.env.remove(&var_name);
                    }
                }
            }
            // No frames left means execution is complete
            Step::Done
        }

        Control::Suspend(_) => {
            // Suspend: DO NOT unwind the stack
            // The VM is now in a suspended state with all frames preserved
            // Execution stops here and can be resumed later
            Step::Done
        }

        Control::None => {
            // Should never happen - unwind is only called when control != None
            panic!("Internal error: unwind() called with Control::None");
        }

        Control::Throw(error) => {
            // Throw: Pop frames until we find a try/catch handler
            // Walk the frame stack from top to bottom looking for Try frames
            while let Some(frame) = vm.frames.last() {
                match &frame.kind {
                    super::types::FrameKind::Try {
                        phase: _,
                        catch_var,
                    } => {
                        // Found a try/catch handler!
                        // Bind the error to the catch variable
                        vm.env.insert(catch_var.clone(), error.clone());

                        // Transition this frame to ExecuteCatch phase
                        let frame_idx = vm.frames.len() - 1;
                        vm.frames[frame_idx].kind = super::types::FrameKind::Try {
                            phase: super::types::TryPhase::ExecuteCatch,
                            catch_var: catch_var.clone(),
                        };

                        // Clear the error control flow
                        vm.control = super::types::Control::None;

                        // Continue execution (will run the catch block)
                        return Step::Continue;
                    }
                    _ => {
                        // Not a try/catch handler, pop this frame and clean up if needed
                        let frame = vm.frames.pop().unwrap();
                        // Clean up any variables declared in this block
                        if let FrameKind::Block { declared_vars, .. } = frame.kind {
                            for var_name in declared_vars {
                                vm.env.remove(&var_name);
                            }
                        }
                    }
                }
            }

            // No try/catch handler found - error propagates to top level
            // Restore the error control (we cleared it in the loop check)
            vm.control = super::types::Control::Throw(error.clone());
            Step::Done
        }

        Control::Break(label) => {
            // Break: Pop frames until we find a matching loop (While/For)
            while let Some(frame) = vm.frames.last() {
                match &frame.kind {
                    super::types::FrameKind::While {
                        phase: _,
                        label: loop_label,
                    } => {
                        // Check if this is the target loop
                        if label.is_none() || label == loop_label {
                            // Found the target loop - pop it and clear control flow
                            vm.frames.pop();
                            vm.control = Control::None;
                            return Step::Continue;
                        } else {
                            // Not the target loop - pop and continue searching
                            let frame = vm.frames.pop().unwrap();
                            // Clean up any variables declared in this block
                            if let FrameKind::Block { declared_vars, .. } = frame.kind {
                                for var_name in declared_vars {
                                    vm.env.remove(&var_name);
                                }
                            }
                        }
                    }
                    _ => {
                        // Not a loop frame - pop it and clean up if needed
                        let frame = vm.frames.pop().unwrap();
                        // Clean up any variables declared in this block
                        if let FrameKind::Block { declared_vars, .. } = frame.kind {
                            for var_name in declared_vars {
                                vm.env.remove(&var_name);
                            }
                        }
                    }
                }
            }

            // No matching loop found - this is an error
            // Restore the break control and signal done (error at top level)
            vm.control = Control::Break(label.clone());
            Step::Done
        }

        Control::Continue(label) => {
            // Continue: Pop frames until we find a matching loop, then reset it to re-evaluate test
            // First, collect frames to pop (everything above the target loop)
            let mut frames_to_pop = 0;
            let mut found_loop = false;

            for i in (0..vm.frames.len()).rev() {
                match &vm.frames[i].kind {
                    super::types::FrameKind::While {
                        phase: _,
                        label: loop_label,
                    } => {
                        // Check if this is the target loop
                        if label.is_none() || label == loop_label {
                            // Found target loop!
                            found_loop = true;
                            // Don't include this frame in frames_to_pop
                            break;
                        } else {
                            // Not the target loop - count it for popping
                            frames_to_pop += 1;
                        }
                    }
                    _ => {
                        // Not a loop frame - count it for popping
                        frames_to_pop += 1;
                    }
                }
            }

            if found_loop {
                // Pop all frames above the target loop, cleaning up blocks as we go
                for _ in 0..frames_to_pop {
                    let frame = vm.frames.pop().unwrap();
                    // Clean up any variables declared in this block
                    if let FrameKind::Block { declared_vars, .. } = frame.kind {
                        for var_name in declared_vars {
                            vm.env.remove(&var_name);
                        }
                    }
                }

                // The target loop is now at the top
                // We don't need to reset its phase - it's already in Eval phase
                // (WhilePhase only has one phase: Eval)

                // Clear control flow and continue
                vm.control = Control::None;
                Step::Continue
            } else {
                // No matching loop found - this is an error
                vm.control = Control::Continue(label.clone());
                Step::Done
            }
        }
    }
}
