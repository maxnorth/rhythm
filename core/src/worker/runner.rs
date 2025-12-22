//! V2 Workflow Runner

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::PgPool;

use super::complete::finish_work;
use crate::db;
use crate::executor::{
    json_to_val, json_to_val_map, run_until_done, val_map_to_json, val_to_json, Control, VM,
};
use crate::parser::parse_workflow;
use crate::types::{CreateExecutionParams, ExecutionOutcome, ExecutionType};

pub async fn run_workflow(pool: &PgPool, execution: crate::types::Execution) -> Result<()> {
    let maybe_context = db::workflow_execution_context::get_context(pool, &execution.id).await?;

    let (mut vm, workflow_def_id) = if let Some(context) = maybe_context {
        (
            serde_json::from_value(context.vm_state).context("Failed to deserialize VM state")?,
            context.workflow_definition_id,
        )
    } else {
        initialize_workflow(pool, &execution.target_name, &execution.inputs).await?
    };

    loop {
        // if suspended and has result, or any other status
        if !try_resume_suspended_task(pool, &mut vm).await? {
            break; // Task not ready, suspend and save state
        }

        run_until_done(&mut vm);

        if !should_continue_execution(&vm.control)? {
            break; // Workflow completed or errored
        }
    }

    let mut tx = pool.begin().await?;
    create_child_tasks(&mut tx, &vm.outbox, &execution.id, &execution.queue).await?;
    handle_workflow_result(&mut tx, &vm, &execution.id, workflow_def_id).await?;
    tx.commit().await?;

    Ok(())
}

/// Checks if VM is suspended on a completed task and resumes if so.
/// Returns true if execution should continue, false if it should break.
async fn try_resume_suspended_task(pool: &PgPool, vm: &mut VM) -> Result<bool> {
    if let Control::Suspend(task_id) = &vm.control {
        if let Some(task_execution) = db::executions::get_execution(pool, task_id).await? {
            match task_execution.status {
                crate::types::ExecutionStatus::Completed
                | crate::types::ExecutionStatus::Failed => {
                    let task_result = task_execution
                        .output
                        .map(|json| json_to_val(&json))
                        .transpose()?
                        .unwrap_or(crate::executor::Val::Null);

                    vm.resume(task_result);
                    Ok(true) // Continue execution
                }
                _ => {
                    Ok(false) // Task still pending, break
                }
            }
        } else {
            // Task doesn't exist in database - check if it's in the outbox (just created)
            let task_in_outbox = vm.outbox.iter().any(|t| t.task_id == *task_id);
            if task_in_outbox {
                Ok(false) // Task in outbox, break to save it
            } else {
                Err(anyhow::anyhow!(
                    "Workflow suspended on non-existent task: {}",
                    task_id
                ))
            }
        }
    } else {
        Ok(true) // Not suspended, continue
    }
}

/// Checks the VM control state and returns whether to continue the loop.
fn should_continue_execution(control: &Control) -> Result<bool> {
    match control {
        Control::None | Control::Suspend(_) => Ok(true), // Still running, continue
        Control::Return(_) | Control::Throw(_) => Ok(false), // Suspend/complete, break
        Control::Break(_) | Control::Continue(_) => {
            Err(anyhow::anyhow!("Unexpected control flow at top level"))
        }
    }
}

async fn initialize_workflow(
    pool: &PgPool,
    workflow_name: &str,
    inputs: &JsonValue,
) -> Result<(VM, i32)> {
    let (workflow_def_id, workflow_source) =
        db::workflow_definitions::get_workflow_by_name(pool, workflow_name).await?;

    let workflow_def = parse_workflow(&workflow_source)
        .map_err(|e| anyhow::anyhow!("Failed to parse workflow: {:?}", e))?;

    let workflow_inputs = json_to_val_map(inputs)?;
    let vm = VM::new(workflow_def.body, workflow_inputs);

    Ok((vm, workflow_def_id))
}

async fn create_child_tasks(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    outbox: &[crate::executor::TaskCreation],
    execution_id: &str,
    queue: &str,
) -> Result<()> {
    if outbox.is_empty() {
        return Ok(());
    }

    for task_creation in outbox {
        let task_inputs = val_map_to_json(&task_creation.inputs)?;

        let params = CreateExecutionParams {
            id: Some(task_creation.task_id.clone()),
            exec_type: ExecutionType::Task,
            target_name: task_creation.task_name.clone(),
            queue: queue.to_string(),
            inputs: task_inputs,
            parent_workflow_id: Some(execution_id.to_string()),
        };

        db::executions::create_execution(tx, params)
            .await
            .context("Failed to create child task execution")?;

        db::work_queue::enqueue_work(&mut **tx, &task_creation.task_id, queue, 0)
            .await
            .context("Failed to enqueue work")?;
    }

    Ok(())
}

async fn handle_workflow_result(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    vm: &VM,
    execution_id: &str,
    workflow_def_id: i32,
) -> Result<()> {
    match &vm.control {
        Control::Return(val) => {
            let result_json = val_to_json(val)?;

            // Delete workflow execution context before finishing
            db::workflow_execution_context::delete_context(&mut **tx, execution_id)
                .await
                .context("Failed to delete workflow execution context")?;

            // Use helper to complete execution, complete work, and re-queue parent
            finish_work(
                &mut *tx,
                execution_id,
                ExecutionOutcome::Success(result_json),
            )
            .await?;
        }
        Control::Suspend(_task_id) => {
            let vm_state = serde_json::to_value(vm).context("Failed to serialize VM state")?;

            // Upsert workflow execution context before suspending
            db::workflow_execution_context::upsert_context(
                tx,
                execution_id,
                workflow_def_id,
                &vm_state,
            )
            .await
            .context("Failed to upsert workflow execution context")?;

            // Use helper to suspend execution, complete work, and re-queue parent
            finish_work(&mut *tx, execution_id, ExecutionOutcome::Suspended).await?;
        }
        Control::Throw(error_val) => {
            let error_json = val_to_json(error_val)?;

            // Delete workflow execution context before finishing
            db::workflow_execution_context::delete_context(&mut **tx, execution_id)
                .await
                .context("Failed to delete workflow execution context")?;

            // Use helper to fail execution, complete work, and re-queue parent
            finish_work(
                &mut *tx,
                execution_id,
                ExecutionOutcome::Failure(error_json),
            )
            .await?;

            return Err(anyhow::anyhow!("Workflow threw error: {:?}", error_val));
        }
        _ => {
            let error_json = serde_json::json!({
                "message": format!("Unexpected control state: {:?}", vm.control),
                "type": "UnexpectedControlState"
            });

            // Delete workflow execution context before finishing
            db::workflow_execution_context::delete_context(&mut **tx, execution_id)
                .await
                .context("Failed to delete workflow execution context")?;

            // Use helper to fail execution, complete work, and re-queue parent
            finish_work(
                &mut *tx,
                execution_id,
                ExecutionOutcome::Failure(error_json),
            )
            .await?;

            return Err(anyhow::anyhow!(
                "Unexpected control state: {:?}",
                vm.control
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
