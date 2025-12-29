//! V2 Workflow Runner

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::PgPool;

use super::awaitable::{resolve_awaitable, AwaitableStatus};
use super::complete::finish_work;
use super::signals::{
    match_outbox_signals_to_unclaimed, process_signal_outbox, resolve_signal_claims,
};
use crate::db;
use crate::executor::{json_to_val_map, run_until_done, val_map_to_json, val_to_json, Control, VM};
use crate::parser::parse_workflow;
use crate::types::{CreateExecutionParams, ExecutionOutcome, ExecutionType};

pub async fn run_workflow(pool: &PgPool, execution: crate::types::Execution) -> Result<()> {
    let maybe_context = db::workflow_execution_context::get_context(pool, &execution.id).await?;

    let (mut vm, workflow_def_id) = if let Some(context) = maybe_context {
        // Resuming a workflow - resolve any signal race conditions from previous runs
        resolve_signal_claims(pool, &execution.id).await?;

        (
            serde_json::from_value(context.vm_state).context("Failed to deserialize VM state")?,
            context.workflow_definition_id,
        )
    } else {
        initialize_workflow(pool, &execution.target_name, &execution.inputs).await?
    };

    loop {
        // Fetch current DB time for timer resolution checks
        let db_now = db::get_db_time(pool).await?;

        // If suspended on an awaitable, check if it's ready
        if !try_resume_suspended_state(pool, &mut vm, db_now).await? {
            break; // Awaitable not ready, suspend and save state
        }

        run_until_done(&mut vm);

        // Match outbox signals to unclaimed DB signals (in-memory, no writes)
        match_outbox_signals_to_unclaimed(pool, &mut vm.outbox, &execution.id).await?;

        if !should_continue_execution(&vm.control)? {
            break; // Workflow completed or errored
        }
    }

    let mut tx = pool.begin().await?;
    create_child_tasks(&mut tx, &vm.outbox, &execution.id, &execution.queue).await?;
    schedule_timers(&mut tx, &vm.outbox, &execution.id, &execution.queue).await?;
    process_signal_outbox(&mut tx, &vm.outbox, &execution.id).await?;
    handle_workflow_result(&mut tx, &vm, &execution.id, workflow_def_id).await?;
    tx.commit().await?;

    Ok(())
}

/// Checks if VM is suspended on a completed awaitable and resumes if so.
/// Returns true if execution should continue, false if it should break.
async fn try_resume_suspended_state(
    pool: &PgPool,
    vm: &mut VM,
    db_now: DateTime<Utc>,
) -> Result<bool> {
    if let Control::Suspend(awaitable) = &vm.control {
        // Clone to avoid borrow issues
        let awaitable = awaitable.clone();

        match resolve_awaitable(pool, &awaitable, db_now, &vm.outbox).await? {
            AwaitableStatus::Pending => Ok(false),
            AwaitableStatus::Success(val) | AwaitableStatus::Error(val) => {
                vm.resume(val);
                Ok(true)
            }
        }
    } else {
        Ok(true) // Not suspended, continue
    }
}

/// Checks the VM control state and returns whether to continue the loop.
fn should_continue_execution(control: &Control) -> Result<bool> {
    match control {
        Control::None => Ok(false), // Workflow completed (implicit return null)
        Control::Suspend(_) => Ok(true), // Still running, awaiting something
        Control::Return(_) | Control::Throw(_) => Ok(false), // Explicit return/throw
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
    outbox: &crate::executor::Outbox,
    execution_id: &str,
    queue: &str,
) -> Result<()> {
    if outbox.tasks.is_empty() {
        return Ok(());
    }

    for task_creation in &outbox.tasks {
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

async fn schedule_timers(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    outbox: &crate::executor::Outbox,
    execution_id: &str,
    queue: &str,
) -> Result<()> {
    use crate::services::scheduler_service::ScheduledParams;

    if outbox.timers.is_empty() {
        return Ok(());
    }

    for timer in &outbox.timers {
        let params = ScheduledParams::WorkflowContinuation {
            execution_id: execution_id.to_string(),
            queue: queue.to_string(),
            priority: 0,
        };

        let params_json =
            serde_json::to_value(&params).context("Failed to serialize scheduled params")?;

        // Convert DateTime<Utc> to NaiveDateTime for the DB
        let run_at = timer.fire_at.naive_utc();

        db::scheduled_queue::schedule_item(&mut **tx, run_at, &params_json)
            .await
            .context("Failed to schedule timer")?;
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
        Control::None => {
            // Implicit return null - workflow completed without explicit return statement
            db::workflow_execution_context::delete_context(&mut **tx, execution_id)
                .await
                .context("Failed to delete workflow execution context")?;

            finish_work(
                &mut *tx,
                execution_id,
                ExecutionOutcome::Success(serde_json::json!(null)),
            )
            .await?;
        }
        Control::Suspend(_awaitable) => {
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
