//! Statement execution handlers
//!
//! Each statement type has its own handler function that processes
//! the statement based on its current execution phase.

use super::errors::ErrorInfo;
use super::expressions::{eval_expr, EvalResult};
use super::stdlib::to_string;
use super::types::{
    AssignPhase, BlockPhase, BreakPhase, ContinuePhase, Control, DeclarePhase, Expr, ExprPhase,
    FrameKind, IfPhase, MemberAccess, ReturnPhase, Stmt, TryPhase, Val, VarKind, WhilePhase,
};
use super::vm::{push_stmt, Step, VM};

/* ===================== Statement Handlers ===================== */

/// Execute Block statement
pub fn execute_block(
    vm: &mut VM,
    phase: BlockPhase,
    idx: usize,
    mut declared_vars: Vec<String>,
    body: &[Stmt],
) -> Step {
    match phase {
        BlockPhase::Execute => {
            // Check if we've finished all statements in the block
            if idx >= body.len() {
                // Block complete, clean up declared variables
                for var_name in declared_vars.iter() {
                    vm.env.remove(var_name);
                }

                // Pop frame
                vm.frames.pop();
                return Step::Continue;
            }

            // Get the current statement to execute
            let child_stmt = &body[idx];

            // If this is a declaration, track it for cleanup
            if let Stmt::Declare { name, .. } = child_stmt {
                declared_vars.push(name.clone());
            }

            // Update our frame to point to the next statement
            let frame_idx = vm.frames.len() - 1;
            vm.frames[frame_idx].kind = FrameKind::Block {
                phase: BlockPhase::Execute,
                idx: idx + 1,
                declared_vars,
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
                match eval_expr(&expr, &vm.env, &mut vm.resume_value, &mut vm.outbox) {
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
    _catch_var: String,
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

/// Execute Expr statement
pub fn execute_expr(vm: &mut VM, phase: ExprPhase, expr: Expr) -> Step {
    match phase {
        ExprPhase::Eval => {
            // Evaluate the expression
            match eval_expr(&expr, &vm.env, &mut vm.resume_value, &mut vm.outbox) {
                EvalResult::Value { .. } => {
                    // Expression evaluated successfully
                    // Discard the result (expression statements don't produce values)
                    // Pop this frame and continue
                    vm.frames.pop();
                    Step::Continue
                }
                EvalResult::Suspend { task_id } => {
                    // Expression suspended (await encountered)
                    // Set control to Suspend and stop execution
                    // DO NOT pop the frame - we need to preserve state for resumption
                    vm.control = Control::Suspend(task_id);
                    Step::Done
                }
                EvalResult::Throw { error } => {
                    // Expression threw an error
                    // Set control to Throw and DO NOT pop frame (unwinding will handle it)
                    vm.control = Control::Throw(error);
                    Step::Continue
                }
            }
        }
    }
}

/// Execute Assign statement
pub fn execute_assign(
    vm: &mut VM,
    phase: AssignPhase,
    var: String,
    path: Vec<MemberAccess>,
    value: Expr,
) -> Step {
    match phase {
        AssignPhase::Eval => {
            // Step 1: Evaluate all path segment expressions and build the access path
            // We need to track both the keys and the segment types for runtime validation
            let mut path_segments: Vec<(String, bool)> = Vec::new(); // (key, is_prop)
            for segment in &path {
                match segment {
                    MemberAccess::Prop { property } => {
                        // Static property - use as-is
                        path_segments.push((property.clone(), true));
                    }
                    MemberAccess::Index { expr } => {
                        // Evaluate the index expression and convert to string key
                        match eval_expr(expr, &vm.env, &mut vm.resume_value, &mut vm.outbox) {
                            EvalResult::Value { v } => {
                                path_segments.push((to_string(&v), false));
                            }
                            EvalResult::Suspend { .. } => {
                                // Should never happen - semantic validator ensures no await in paths
                                panic!("Internal error: await in assignment path");
                            }
                            EvalResult::Throw { error } => {
                                // Index expression threw an error
                                vm.control = Control::Throw(error);
                                return Step::Continue;
                            }
                        }
                    }
                }
            }

            // Step 2: Evaluate the value expression
            let value_result =
                match eval_expr(&value, &vm.env, &mut vm.resume_value, &mut vm.outbox) {
                    EvalResult::Value { v } => v,
                    EvalResult::Suspend { task_id } => {
                        // Expression suspended (await encountered)
                        // For simple assignment (empty path), this is allowed
                        // For attribute assignment (non-empty path), semantic validator should prevent this
                        if !path_segments.is_empty() {
                            panic!("Internal error: await in attribute assignment value");
                        }
                        vm.control = Control::Suspend(task_id);
                        return Step::Done;
                    }
                    EvalResult::Throw { error } => {
                        // Value expression threw an error
                        vm.control = Control::Throw(error);
                        return Step::Continue;
                    }
                };

            // Step 3: Perform the assignment
            if path_segments.is_empty() {
                // Simple assignment: x = value
                vm.env.insert(var, value_result);
            } else {
                // Attribute assignment: obj.prop = value or arr[i] = value
                // Get the base object from the environment
                let base = match vm.env.get_mut(&var) {
                    Some(v) => v,
                    None => {
                        // Variable doesn't exist
                        vm.control = Control::Throw(Val::Error(ErrorInfo {
                            code: "ReferenceError".to_string(),
                            message: format!("Variable '{}' is not defined", var),
                        }));
                        return Step::Continue;
                    }
                };

                // Walk the path, navigating to the container that holds the final property
                let mut current = base;
                for (key, is_prop) in &path_segments[..path_segments.len() - 1] {
                    // Validate access type matches value type
                    if *is_prop {
                        // Prop access - only valid on objects
                        if !matches!(current, Val::Obj(_)) {
                            vm.control = Control::Throw(Val::Error(ErrorInfo {
                                code: "TypeError".to_string(),
                                message: format!(
                                    "Cannot access property '{}' on non-object value",
                                    key
                                ),
                            }));
                            return Step::Continue;
                        }
                    } else {
                        // Index access - valid on objects and arrays
                        if !matches!(current, Val::Obj(_) | Val::List(_)) {
                            vm.control = Control::Throw(Val::Error(ErrorInfo {
                                code: "TypeError".to_string(),
                                message: "Cannot use index access on non-object/non-array value"
                                    .to_string(),
                            }));
                            return Step::Continue;
                        }
                    }

                    current = match current {
                        Val::Obj(map) => match map.get_mut(key) {
                            Some(v) => v,
                            None => {
                                vm.control = Control::Throw(Val::Error(ErrorInfo {
                                    code: "TypeError".to_string(),
                                    message: format!("Cannot read property '{}' of undefined", key),
                                }));
                                return Step::Continue;
                            }
                        },
                        Val::List(arr) => {
                            // Try to parse key as number
                            match key.parse::<usize>() {
                                Ok(idx) if idx < arr.len() => &mut arr[idx],
                                _ => {
                                    vm.control = Control::Throw(Val::Error(ErrorInfo {
                                        code: "TypeError".to_string(),
                                        message: format!("Invalid array index: {}", key),
                                    }));
                                    return Step::Continue;
                                }
                            }
                        }
                        _ => {
                            // This should never happen due to validation above
                            unreachable!();
                        }
                    };
                }

                // Set the final property with type validation
                let (final_key, is_prop) = &path_segments[path_segments.len() - 1];

                // Validate access type matches value type
                if *is_prop {
                    // Prop access - only valid on objects
                    if !matches!(current, Val::Obj(_)) {
                        vm.control = Control::Throw(Val::Error(ErrorInfo {
                            code: "TypeError".to_string(),
                            message: format!(
                                "Cannot set property '{}' on non-object value",
                                final_key
                            ),
                        }));
                        return Step::Continue;
                    }
                } else {
                    // Index access - valid on objects and arrays
                    if !matches!(current, Val::Obj(_) | Val::List(_)) {
                        vm.control = Control::Throw(Val::Error(ErrorInfo {
                            code: "TypeError".to_string(),
                            message: "Cannot use index access on non-object/non-array value"
                                .to_string(),
                        }));
                        return Step::Continue;
                    }
                }

                match current {
                    Val::Obj(map) => {
                        map.insert(final_key.clone(), value_result);
                    }
                    Val::List(arr) => {
                        // Try to parse key as number
                        match final_key.parse::<usize>() {
                            Ok(idx) if idx < arr.len() => {
                                arr[idx] = value_result;
                            }
                            _ => {
                                vm.control = Control::Throw(Val::Error(ErrorInfo {
                                    code: "TypeError".to_string(),
                                    message: format!("Invalid array index: {}", final_key),
                                }));
                                return Step::Continue;
                            }
                        }
                    }
                    _ => {
                        // This should never happen due to validation above
                        unreachable!();
                    }
                }
            }

            // Pop this frame and continue
            vm.frames.pop();
            Step::Continue
        }
    }
}

/// Execute If statement
pub fn execute_if(
    vm: &mut VM,
    phase: IfPhase,
    test: Expr,
    then_s: Box<Stmt>,
    else_s: Option<Box<Stmt>>,
) -> Step {
    match phase {
        IfPhase::Eval => {
            // Evaluate the test expression
            let test_val = match eval_expr(&test, &vm.env, &mut vm.resume_value, &mut vm.outbox) {
                EvalResult::Value { v } => v,
                EvalResult::Suspend { .. } => {
                    // Should never happen - semantic validator ensures no await in test
                    panic!("Internal error: await in if test expression");
                }
                EvalResult::Throw { error } => {
                    // Test expression threw an error
                    vm.control = Control::Throw(error);
                    return Step::Continue;
                }
            };

            // Check truthiness to decide which branch to execute
            let is_truthy = test_val.is_truthy();

            // Pop this If frame
            vm.frames.pop();

            // Push the appropriate branch onto the stack
            if is_truthy {
                // Execute then branch
                push_stmt(vm, &then_s);
            } else if let Some(else_stmt) = &else_s {
                // Execute else branch if it exists
                push_stmt(vm, else_stmt);
            }
            // If not truthy and no else branch, we just continue (nothing to execute)

            Step::Continue
        }
    }
}

/// Execute While statement
pub fn execute_while(vm: &mut VM, phase: WhilePhase, test: Expr, body: Box<Stmt>) -> Step {
    match phase {
        WhilePhase::Eval => {
            // Evaluate the test expression
            let test_val = match eval_expr(&test, &vm.env, &mut vm.resume_value, &mut vm.outbox) {
                EvalResult::Value { v } => v,
                EvalResult::Suspend { .. } => {
                    // Should never happen - semantic validator ensures no await in test
                    panic!("Internal error: await in while test expression");
                }
                EvalResult::Throw { error } => {
                    // Test expression threw an error
                    vm.control = Control::Throw(error);
                    return Step::Continue;
                }
            };

            // Check truthiness to decide whether to continue the loop
            let is_truthy = test_val.is_truthy();

            if is_truthy {
                // Continue looping - keep the While frame on the stack and push the body
                push_stmt(vm, &body);
                Step::Continue
            } else {
                // Loop finished - pop this While frame
                vm.frames.pop();
                Step::Continue
            }
        }
    }
}

/// Execute Break statement
pub fn execute_break(vm: &mut VM, _phase: BreakPhase) -> Step {
    // Set control flow to Break (no label support yet)
    vm.control = Control::Break(None);
    // Pop this Break frame
    vm.frames.pop();
    Step::Continue
}

/// Execute Continue statement
pub fn execute_continue(vm: &mut VM, _phase: ContinuePhase) -> Step {
    // Set control flow to Continue (no label support yet)
    vm.control = Control::Continue(None);
    // Pop this Continue frame
    vm.frames.pop();
    Step::Continue
}

/// Execute Declare statement (let/const)
pub fn execute_declare(
    vm: &mut VM,
    phase: DeclarePhase,
    _var_kind: VarKind,
    name: String,
    init: Option<Expr>,
) -> Step {
    match phase {
        DeclarePhase::Eval => {
            // Evaluate the initialization expression (if present) or use null
            let value = if let Some(expr) = init {
                match eval_expr(&expr, &vm.env, &mut vm.resume_value, &mut vm.outbox) {
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
                // No initialization expression - default to null
                Val::Null
            };

            // Insert the variable into the environment
            // Note: Shadowing checks are handled by the semantic validator
            // Note: The parent block frame has already added this variable to its declared_vars list
            vm.env.insert(name, value);

            // Pop this frame and continue
            vm.frames.pop();
            Step::Continue
        }
    }
}
