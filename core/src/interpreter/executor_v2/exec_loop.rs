//! Core execution loop
//!
//! This module contains the step() function - the heart of the interpreter.
//! It processes one frame at a time, advancing execution phases and managing the frame stack.
//!
//! ## Function Organization
//! Functions are ordered by importance/call hierarchy:
//! 1. run_until_done() - Top-level driver (calls step repeatedly)
//! 2. step() - Main execution loop (dispatches to statement handlers)

use super::statements::{execute_block, execute_return, execute_try};
use super::types::{Control, FrameKind, Stmt};
use super::vm::{Step, VM};

/* ===================== Public API ===================== */

/// Run the VM until it completes
///
/// This is the top-level driver that repeatedly calls step() until execution finishes.
/// After completion, inspect `vm.control` for the final state.
pub fn run_until_done(vm: &mut VM) {
    loop {
        match step(vm) {
            Step::Continue => continue,
            Step::Done => break,
        }
    }
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
        (FrameKind::Return { phase }, Stmt::Return { value }) => {
            execute_return(vm, phase, value)
        }

        (FrameKind::Block { phase, idx }, Stmt::Block { body }) => {
            execute_block(vm, phase, idx, body)
        }

        (
            FrameKind::Try { phase, catch_var },
            Stmt::Try {
                body,
                catch_var: _,
                catch_body,
            },
        ) => execute_try(vm, phase, catch_var, body, catch_body),

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
            // Pop all frames - return exits the entire program
            vm.frames.clear();
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
                        phase,
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
                        // Not a try/catch handler, pop this frame and continue
                        vm.frames.pop();
                    }
                }
            }

            // No try/catch handler found - error propagates to top level
            // Restore the error control (we cleared it in the loop check)
            vm.control = super::types::Control::Throw(error.clone());
            Step::Done
        }

        Control::Break | Control::Continue => {
            // Not yet implemented - will be added in later milestones
            panic!("Break/Continue not yet implemented");
        }
    }
}
