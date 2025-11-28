//! V2 Workflow Runner

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::PgPool;

use crate::db;
use crate::types::{CreateExecutionParams, ExecutionOutcome, ExecutionType};
use crate::executor::{
    json_to_val, json_to_val_map, run_until_done, val_map_to_json, val_to_json, Control, VM,
};
use crate::parser::parse_workflow;
use super::complete::finish_work;

pub async fn run_workflow(pool: &PgPool, execution: crate::types::Execution) -> Result<()> {
    let maybe_context = db::workflow_execution_context::get_context(&pool, &execution.id).await?;

    let (mut vm, workflow_def_id) = if let Some(context) = maybe_context {
        resume_workflow(&pool, context).await?
    } else {
        initialize_workflow(&pool, &execution.function_name, &execution.inputs).await?
    };

    run_until_done(&mut vm);

    let mut tx = pool.begin().await?;

    create_child_tasks(&mut tx, &vm.outbox, &execution.id, &execution.queue).await?;

    handle_workflow_result(
        &mut tx,
        &vm,
        &execution.id,
        workflow_def_id,
    )
    .await?;

    tx.commit().await?;

    Ok(())
}

async fn resume_workflow(
    pool: &PgPool,
    context: db::workflow_execution_context::WorkflowExecutionContext,
) -> Result<(VM, i32)> {
    let mut vm: VM =
        serde_json::from_value(context.vm_state).context("Failed to deserialize VM state")?;

    let task_id = match &vm.control {
        Control::Suspend(id) => id.clone(),
        _ => {
            return Err(anyhow::anyhow!(
                "Workflow execution context exists but VM is not suspended"
            ));
        }
    };

    let task_execution = db::executions::get_execution(&pool, &task_id)
        .await
        .context("Failed to fetch suspended task execution")?
        .ok_or_else(|| anyhow::anyhow!("Task execution not found: {}", task_id))?;

    let task_result = task_execution
        .output
        .ok_or_else(|| anyhow::anyhow!("Task execution has no result"))?;

    let task_result_val = json_to_val(&task_result)?;

    if !vm.resume(task_result_val) {
        return Err(anyhow::anyhow!("Failed to resume VM"));
    }

    Ok((vm, context.workflow_definition_id))
}

async fn initialize_workflow(pool: &PgPool, workflow_name: &str, inputs: &JsonValue) -> Result<(VM, i32)> {
    let (workflow_def_id, workflow_source) =
        db::workflow_definitions::get_workflow_by_name(&pool, workflow_name).await?;

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
    for task_creation in outbox {
        let task_inputs = val_map_to_json(&task_creation.inputs)?;

        let params = CreateExecutionParams {
            id: Some(task_creation.task_id.clone()),
            exec_type: ExecutionType::Task,
            function_name: task_creation.task_name.clone(),
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
            let result_json = val_to_json(&val)?;

            // Delete workflow execution context before finishing
            db::workflow_execution_context::delete_context(&mut **tx, &execution_id)
                .await
                .context("Failed to delete workflow execution context")?;

            // Use helper to complete execution, complete work, and re-queue parent
            finish_work(&mut *tx, &execution_id, ExecutionOutcome::Success(result_json))
                .await?;
        }
        Control::Suspend(_) => {
            let vm_state = serde_json::to_value(&vm)
                .context("Failed to serialize VM state")?;

            // Upsert workflow execution context before suspending
            db::workflow_execution_context::upsert_context(tx, &execution_id, workflow_def_id, &vm_state)
                .await
                .context("Failed to upsert workflow execution context")?;

            // Use helper to suspend execution, complete work, and re-queue parent
            finish_work(
                &mut *tx,
                &execution_id,
                ExecutionOutcome::Suspended
            )
            .await?;
        }
        Control::Throw(error_val) => {
            let error_json = val_to_json(&error_val)?;

            // Delete workflow execution context before finishing
            db::workflow_execution_context::delete_context(&mut **tx, &execution_id)
                .await
                .context("Failed to delete workflow execution context")?;

            // Use helper to fail execution, complete work, and re-queue parent
            finish_work(&mut *tx, &execution_id, ExecutionOutcome::Failure(error_json))
                .await?;

            return Err(anyhow::anyhow!("Workflow threw error: {:?}", error_val));
        }
        _ => {
            let error_json = serde_json::json!({
                "message": format!("Unexpected control state: {:?}", vm.control),
                "type": "UnexpectedControlState"
            });

            // Delete workflow execution context before finishing
            db::workflow_execution_context::delete_context(&mut **tx, &execution_id)
                .await
                .context("Failed to delete workflow execution context")?;

            // Use helper to fail execution, complete work, and re-queue parent
            finish_work(&mut *tx, &execution_id, ExecutionOutcome::Failure(error_json))
                .await?;

            return Err(anyhow::anyhow!("Unexpected control state: {:?}", vm.control));
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
