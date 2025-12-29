//! Statement execution handlers
//!
//! Each statement type has its own handler function that processes
//! the statement based on its current execution phase.

use super::errors::{self, ErrorInfo};
use super::expressions::{eval_expr, EvalResult};
use super::stdlib::to_string;
use super::types::{
    AssignPhase, BlockPhase, BreakPhase, ContinuePhase, Control, DeclarePhase, DeclareTarget, Expr,
    ExprPhase, ForLoopKind, ForLoopPhase, FrameKind, IfPhase, MemberAccess, ReturnPhase, Stmt,
    TryPhase, Val, VarKind, WhilePhase,
};
use super::vm::{push_stmt, VM};

/* ===================== Statement Handlers ===================== */

/// Execute Block statement
pub fn execute_block(
    vm: &mut VM,
    phase: BlockPhase,
    idx: usize,
    declared_vars: Vec<String>,
    body: &[Stmt],
) {
    // If control flow is active, clean up and pop
    if vm.control != Control::None {
        for var_name in &declared_vars {
            vm.env.remove(var_name);
        }
        vm.frames.pop();
        return;
    }

    let mut declared_vars = declared_vars;

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
                return;
            }

            // Get the current statement to execute
            let child_stmt = &body[idx];

            // If this is a declaration, track declared names for cleanup
            if let Stmt::Declare { target, .. } = child_stmt {
                match target {
                    DeclareTarget::Simple { name } => {
                        declared_vars.push(name.clone());
                    }
                    DeclareTarget::Destructure { names } => {
                        declared_vars.extend(names.clone());
                    }
                }
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
        }
    }
}

/// Execute Return statement
pub fn execute_return(vm: &mut VM, phase: ReturnPhase, value: Option<Expr>) {
    match phase {
        ReturnPhase::Eval => {
            // Evaluate the return value (if any)
            let val = if let Some(expr) = value {
                match eval_expr(&expr, &vm.env, &mut vm.resume_value, &mut vm.outbox) {
                    EvalResult::Value { v } => v,
                    EvalResult::Suspend { awaitable } => {
                        // Expression suspended (await encountered)
                        // Set control to Suspend and stop execution
                        // DO NOT pop the frame - we need to preserve state for resumption
                        vm.control = Control::Suspend(awaitable);
                        return;
                    }
                    EvalResult::Throw { error } => {
                        vm.control = Control::Throw(error);
                        vm.frames.pop();
                        return;
                    }
                }
            } else {
                Val::Null
            };

            // Set control to Return
            vm.control = Control::Return(val);

            // Pop this frame
            vm.frames.pop();
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
) {
    // Handle Throw in TryStarted - catch the error
    if let Control::Throw(error) = &vm.control {
        if phase == TryPhase::TryStarted {
            let error = error.clone();
            vm.env.insert(catch_var.clone(), error);
            vm.control = Control::None;

            let frame_idx = vm.frames.len() - 1;
            vm.frames[frame_idx].kind = FrameKind::Try {
                phase: TryPhase::CatchStarted,
                catch_var,
            };
            push_stmt(vm, &catch_body);
            return;
        }
    }

    // Any control flow - clean up catch_var if in CatchStarted, then pop and propagate
    if vm.control != Control::None {
        if phase == TryPhase::CatchStarted {
            vm.env.remove(&catch_var);
        }
        vm.frames.pop();
        return;
    }

    match phase {
        TryPhase::NotStarted => {
            // Transition to TryStarted and push the try body
            let frame_idx = vm.frames.len() - 1;
            vm.frames[frame_idx].kind = FrameKind::Try {
                phase: TryPhase::TryStarted,
                catch_var,
            };
            push_stmt(vm, &body);
        }
        TryPhase::TryStarted => {
            // Try body completed successfully - pop frame, we're done
            vm.frames.pop();
        }
        TryPhase::CatchStarted => {
            // Catch body completed - clean up catch_var and pop frame
            vm.env.remove(&catch_var);
            vm.frames.pop();
        }
    }
}

/// Execute Expr statement
pub fn execute_expr(vm: &mut VM, phase: ExprPhase, expr: Expr) {
    match phase {
        ExprPhase::Eval => {
            // Evaluate the expression
            match eval_expr(&expr, &vm.env, &mut vm.resume_value, &mut vm.outbox) {
                EvalResult::Value { .. } => {
                    // Expression evaluated successfully
                    // Discard the result (expression statements don't produce values)
                    // Pop this frame and continue
                    vm.frames.pop();
                }
                EvalResult::Suspend { awaitable } => {
                    // Expression suspended (await encountered)
                    // Set control to Suspend and stop execution
                    // DO NOT pop the frame - we need to preserve state for resumption
                    vm.control = Control::Suspend(awaitable);
                }
                EvalResult::Throw { error } => {
                    vm.control = Control::Throw(error);
                    vm.frames.pop();
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
) {
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
                                vm.control = Control::Throw(error);
                                vm.frames.pop();
                                return;
                            }
                        }
                    }
                }
            }

            // Step 2: Evaluate the value expression
            let value_result =
                match eval_expr(&value, &vm.env, &mut vm.resume_value, &mut vm.outbox) {
                    EvalResult::Value { v } => v,
                    EvalResult::Suspend { awaitable } => {
                        // Expression suspended (await encountered)
                        // For simple assignment (empty path), this is allowed
                        // For attribute assignment (non-empty path), semantic validator should prevent this
                        if !path_segments.is_empty() {
                            panic!("Internal error: await in attribute assignment value");
                        }
                        vm.control = Control::Suspend(awaitable);
                        return;
                    }
                    EvalResult::Throw { error } => {
                        vm.control = Control::Throw(error);
                        vm.frames.pop();
                        return;
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
                        vm.control = Control::Throw(Val::Error(ErrorInfo {
                            code: "ReferenceError".to_string(),
                            message: format!("Variable '{}' is not defined", var),
                        }));
                        vm.frames.pop();
                        return;
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
                            vm.frames.pop();
                            return;
                        }
                    } else {
                        // Index access - valid on objects and arrays
                        if !matches!(current, Val::Obj(_) | Val::List(_)) {
                            vm.control = Control::Throw(Val::Error(ErrorInfo {
                                code: "TypeError".to_string(),
                                message: "Cannot use index access on non-object/non-array value"
                                    .to_string(),
                            }));
                            vm.frames.pop();
                            return;
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
                                vm.frames.pop();
                                return;
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
                                    vm.frames.pop();
                                    return;
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
                        vm.frames.pop();
                        return;
                    }
                } else {
                    // Index access - valid on objects and arrays
                    if !matches!(current, Val::Obj(_) | Val::List(_)) {
                        vm.control = Control::Throw(Val::Error(ErrorInfo {
                            code: "TypeError".to_string(),
                            message: "Cannot use index access on non-object/non-array value"
                                .to_string(),
                        }));
                        vm.frames.pop();
                        return;
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
                                vm.frames.pop();
                                return;
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
) {
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
                    vm.control = Control::Throw(error);
                    vm.frames.pop();
                    return;
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
        }
    }
}

/// Execute While statement
pub fn execute_while(
    vm: &mut VM,
    phase: WhilePhase,
    label: Option<String>,
    test: Expr,
    body: Box<Stmt>,
) {
    // Handle Continue - if label matches (or is None), re-evaluate test
    if let Control::Continue(continue_label) = &vm.control {
        if continue_label.is_none() || continue_label == &label {
            vm.control = Control::None;
        }
    } else if vm.control != Control::None {
        // Handle Break - if label matches (or is None), exit the loop
        if let Control::Break(break_label) = &vm.control {
            if break_label.is_none() || break_label == &label {
                vm.control = Control::None;
            }
        }
        vm.frames.pop();
        return;
    }

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
                    return;
                }
            };

            // Check truthiness to decide whether to continue the loop
            let is_truthy = test_val.is_truthy();

            if is_truthy {
                // Continue looping - keep the While frame on the stack and push the body
                push_stmt(vm, &body);
            } else {
                // Loop finished - pop this While frame
                vm.frames.pop();
            }
        }
    }
}

/// Execute ForLoop statement (for...in / for...of)
#[allow(clippy::too_many_arguments)]
pub fn execute_for_loop(
    vm: &mut VM,
    _phase: ForLoopPhase,
    items: Option<Vec<Val>>,
    idx: usize,
    kind: ForLoopKind,
    binding: String,
    iterable: Expr,
    body: Box<Stmt>,
) {
    // Handle Continue - no labels yet, so any Continue goes to next iteration
    if let Control::Continue(continue_label) = &vm.control {
        if continue_label.is_none() {
            vm.control = Control::None;
        }
    } else if vm.control != Control::None {
        // Handle Break - no labels yet, so any Break exits the loop
        if let Control::Break(break_label) = &vm.control {
            if break_label.is_none() {
                vm.control = Control::None;
            }
        }
        vm.env.remove(&binding);
        vm.frames.pop();
        return;
    }

    // If items is None, we need to evaluate the iterable first
    let items = match items {
        Some(items) => items,
        None => {
            // Evaluate the iterable expression
            let iterable_val =
                match eval_expr(&iterable, &vm.env, &mut vm.resume_value, &mut vm.outbox) {
                    EvalResult::Value { v } => v,
                    EvalResult::Suspend { .. } => {
                        // Should never happen - semantic validator ensures no await in iterable
                        panic!("Internal error: await in for loop iterable expression");
                    }
                    EvalResult::Throw { error } => {
                        vm.control = Control::Throw(error);
                        return;
                    }
                };

            // Extract items based on loop kind
            match kind {
                ForLoopKind::Of => {
                    // for...of iterates over values
                    match iterable_val {
                        Val::List(arr) => arr,
                        _ => {
                            vm.control = Control::Throw(Val::Error(ErrorInfo::new(
                                errors::TYPE_ERROR,
                                "for...of requires an array",
                            )));
                            return;
                        }
                    }
                }
                ForLoopKind::In => {
                    // for...in iterates over keys
                    match iterable_val {
                        Val::Obj(map) => map.keys().cloned().map(Val::Str).collect(),
                        Val::List(arr) => (0..arr.len()).map(|i| Val::Num(i as f64)).collect(),
                        _ => {
                            vm.control = Control::Throw(Val::Error(ErrorInfo::new(
                                errors::TYPE_ERROR,
                                "for...in requires an object or array",
                            )));
                            return;
                        }
                    }
                }
            }
        }
    };

    // Check if we've exhausted all items
    if idx >= items.len() {
        // Loop complete - clean up binding and pop frame
        vm.env.remove(&binding);
        vm.frames.pop();
        return;
    }

    // Set the binding variable to the current item
    let current_item = items[idx].clone();
    vm.env.insert(binding.clone(), current_item);

    // Advance the index for next iteration
    let frame_idx = vm.frames.len() - 1;
    vm.frames[frame_idx].kind = FrameKind::ForLoop {
        phase: ForLoopPhase::Iterate,
        items: Some(items),
        idx: idx + 1,
    };

    // Push the body onto the stack
    push_stmt(vm, &body);
}

/// Execute Break statement
pub fn execute_break(vm: &mut VM, _phase: BreakPhase) {
    // Set control flow to Break (no label support yet)
    vm.control = Control::Break(None);
    // Pop this Break frame
    vm.frames.pop();
}

/// Execute Continue statement
pub fn execute_continue(vm: &mut VM, _phase: ContinuePhase) {
    // Set control flow to Continue (no label support yet)
    vm.control = Control::Continue(None);
    // Pop this Continue frame
    vm.frames.pop();
}

/// Execute Declare statement (let/const)
pub fn execute_declare(
    vm: &mut VM,
    phase: DeclarePhase,
    _var_kind: VarKind,
    target: DeclareTarget,
    init: Option<Expr>,
) {
    match phase {
        DeclarePhase::Eval => {
            // Evaluate the initialization expression (if present) or use null
            let value = if let Some(expr) = init {
                match eval_expr(&expr, &vm.env, &mut vm.resume_value, &mut vm.outbox) {
                    EvalResult::Value { v } => v,
                    EvalResult::Suspend { awaitable } => {
                        // Expression suspended (await encountered)
                        // Set control to Suspend and stop execution
                        // DO NOT pop the frame - we need to preserve state for resumption
                        vm.control = Control::Suspend(awaitable);
                        return;
                    }
                    EvalResult::Throw { error } => {
                        vm.control = Control::Throw(error);
                        vm.frames.pop();
                        return;
                    }
                }
            } else {
                // No initialization expression - default to null
                Val::Null
            };

            // Insert variable(s) into the environment based on target type
            match target {
                DeclareTarget::Simple { name } => {
                    vm.env.insert(name, value);
                }
                DeclareTarget::Destructure { names } => {
                    // Value must be an object for destructuring
                    let obj = match value {
                        Val::Obj(map) => map,
                        _ => {
                            vm.control = Control::Throw(Val::Error(ErrorInfo::new(
                                errors::TYPE_ERROR,
                                "Cannot destructure non-object value",
                            )));
                            vm.frames.pop();
                            return;
                        }
                    };

                    // Extract each named property
                    for name in names {
                        let prop_value = match obj.get(&name).cloned() {
                            Some(v) => v,
                            None => {
                                vm.control = Control::Throw(Val::Error(ErrorInfo::new(
                                    errors::PROPERTY_NOT_FOUND,
                                    format!("Property '{}' not found on object", name),
                                )));
                                vm.frames.pop();
                                return;
                            }
                        };
                        vm.env.insert(name, prop_value);
                    }
                }
            }

            // Pop this frame and continue
            vm.frames.pop();
        }
    }
}
