//! V2 Workflow Runner

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::PgPool;

use crate::db::get_pool;
use crate::v2::db;
use crate::v2::types::{CreateExecutionParams, ExecutionType};
use crate::v2::executor::{
    json_to_val, json_to_val_map, run_until_done, val_map_to_json, val_to_json, Control, VM,
};
use crate::v2::parser::parse_workflow;

pub async fn run_workflow(
    execution_id: String,
    workflow_name: String,
    inputs: JsonValue,
    queue: String,
) -> Result<()> {
    let pool = get_pool().await?;
    let maybe_context = db::workflow_execution_context::get_context(&pool, &execution_id).await?;

    let (mut vm, workflow_def_id) = if let Some(context) = maybe_context {
        resume_workflow(&pool, context).await?
    } else {
        initialize_workflow(&pool, &workflow_name, &inputs).await?
    };

    run_until_done(&mut vm);

    let mut tx = pool.begin().await?;

    create_child_tasks(&mut tx, &vm, &execution_id, &queue).await?;

    match &vm.control {
        Control::Return(val) => {
            let result_json = val_to_json(&val)?;

            db::executions::complete_execution(&mut *tx, &execution_id, result_json)
                .await
                .context("Failed to complete workflow execution")?;

            db::workflow_execution_context::delete_context(&mut tx, &execution_id)
                .await
                .context("Failed to delete workflow execution context")?;

            db::work_queue::complete_work(&mut tx, &execution_id)
                .await
                .context("Failed to complete work queue entry")?;
        }
        Control::Suspend(_) => {
            let vm_state = serde_json::to_value(&vm)
                .context("Failed to serialize VM state")?;

            db::executions::suspend_execution(&mut tx, &execution_id)
                .await
                .context("Failed to suspend execution")?;

            db::workflow_execution_context::upsert_context(&mut tx, &execution_id, workflow_def_id, &vm_state)
                .await
                .context("Failed to upsert workflow execution context")?;

            db::work_queue::complete_work(&mut tx, &execution_id)
                .await
                .context("Failed to complete work queue entry")?;
        }
        Control::Throw(error_val) => {
            let error_json = val_to_json(&error_val)?;

            db::executions::fail_execution(&mut *tx, &execution_id, error_json)
                .await
                .context("Failed to mark workflow as failed")?;

            db::workflow_execution_context::delete_context(&mut tx, &execution_id)
                .await
                .context("Failed to delete workflow execution context")?;

            db::work_queue::complete_work(&mut tx, &execution_id)
                .await
                .context("Failed to complete work queue entry")?;

            return Err(anyhow::anyhow!("Workflow threw error: {:?}", error_val));
        }
        _ => {
            let error_json = serde_json::json!({
                "message": format!("Unexpected control state: {:?}", vm.control),
                "type": "UnexpectedControlState"
            });

            db::executions::fail_execution(&mut *tx, &execution_id, error_json)
                .await
                .context("Failed to mark workflow as failed")?;

            db::workflow_execution_context::delete_context(&mut tx, &execution_id)
                .await
                .context("Failed to delete workflow execution context")?;

            db::work_queue::complete_work(&mut tx, &execution_id)
                .await
                .context("Failed to complete work queue entry")?;

            return Err(anyhow::anyhow!("Unexpected control state: {:?}", vm.control));
        }
    }

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
    vm: &VM,
    execution_id: &str,
    queue: &str,
) -> Result<()> {
    for task_creation in &vm.outbox {
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
    }

    Ok(())
}

#[cfg(test)]
mod tests;
