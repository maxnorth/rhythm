//! Signals Database Operations
//!
//! Provides signal storage and retrieval for workflow human-in-the-loop patterns.
//!
//! ## Design
//!
//! Signals use a bidirectional matching system:
//! - `status = 'requested'`: workflow waiting for a signal
//! - `status = 'sent'`: signal has been sent
//! - `claim_id`: links a request to its claimed signal (NULL = unclaimed)
//!
//! When Signal.next() is called, we insert a 'requested' row. When an external
//! signal arrives, we either match an existing request or insert an unclaimed signal.
//! Race conditions are resolved by `resolve_signal_claims` at workflow resumption.

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::Row;

/// Insert a signal request (workflow waiting for signal)
///
/// Creates a 'requested' row with the given claim_id. The claim_id links
/// this request to the workflow's awaitable so it can be resolved later.
pub async fn insert_signal_request<'e, E>(
    executor: E,
    workflow_id: &str,
    signal_name: &str,
    claim_id: &str,
) -> Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query(
        r#"
        INSERT INTO signals (workflow_id, signal_name, status, claim_id, created_at)
        VALUES ($1, $2, 'requested', $3, NOW())
        "#,
    )
    .bind(workflow_id)
    .bind(signal_name)
    .bind(claim_id)
    .execute(executor)
    .await
    .context("Failed to insert signal request")?;

    Ok(())
}

/// Send a signal to a workflow (called when external signal arrives)
///
/// Inserts an unclaimed 'sent' signal. The caller should always enqueue the
/// workflow for processing - matching is handled by resolve_signal_claims
/// at workflow resumption.
pub async fn send_signal<'e, E>(
    executor: E,
    workflow_id: &str,
    signal_name: &str,
    payload: &JsonValue,
) -> Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query(
        r#"
        INSERT INTO signals (workflow_id, signal_name, status, payload, created_at)
        VALUES ($1, $2, 'sent', $3, NOW())
        "#,
    )
    .bind(workflow_id)
    .bind(signal_name)
    .bind(payload)
    .execute(executor)
    .await
    .context("Failed to send signal")?;

    Ok(())
}

/// Claim a specific signal by its ID
///
/// Sets the claim_id on the signal, linking it to a request.
pub async fn claim_signal<'e, E>(executor: E, signal_id: &str, claim_id: &str) -> Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query(
        r#"
        UPDATE signals
        SET claim_id = $2
        WHERE id = $1::uuid
        "#,
    )
    .bind(signal_id)
    .bind(claim_id)
    .execute(executor)
    .await
    .context("Failed to claim signal")?;

    Ok(())
}

/// Check if a signal request has been claimed
///
/// Returns the payload if the signal has been claimed, None if still waiting.
pub async fn check_signal_claimed<'e, E>(executor: E, claim_id: &str) -> Result<Option<JsonValue>>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let row = sqlx::query(
        r#"
        SELECT payload FROM signals
        WHERE claim_id = $1 AND status = 'sent'
        "#,
    )
    .bind(claim_id)
    .fetch_optional(executor)
    .await
    .context("Failed to check signal status")?;

    Ok(row.map(|r| r.get("payload")))
}

/// Get unclaimed 'sent' signals for a workflow by signal name
///
/// Returns signal IDs in FIFO order (oldest first), limited to the requested count.
pub async fn get_unclaimed_signals_by_name<'e, E>(
    executor: E,
    workflow_id: &str,
    signal_name: &str,
    limit: i64,
) -> Result<Vec<String>>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let rows = sqlx::query(
        r#"
        SELECT id FROM signals
        WHERE workflow_id = $1 AND signal_name = $2 AND status = 'sent' AND claim_id IS NULL
        ORDER BY created_at ASC
        LIMIT $3
        "#,
    )
    .bind(workflow_id)
    .bind(signal_name)
    .bind(limit)
    .fetch_all(executor)
    .await
    .context("Failed to fetch unclaimed signals")?;

    Ok(rows
        .into_iter()
        .map(|row| row.get::<sqlx::types::Uuid, _>("id").to_string())
        .collect())
}

/// Get payload for a specific signal by ID
pub async fn get_signal_payload<'e, E>(executor: E, signal_id: &str) -> Result<JsonValue>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let row = sqlx::query(
        r#"
        SELECT payload FROM signals WHERE id = $1::uuid
        "#,
    )
    .bind(signal_id)
    .fetch_one(executor)
    .await
    .context("Failed to fetch signal payload")?;

    Ok(row.get("payload"))
}

/// A requested signal waiting for a match
#[derive(Debug)]
pub struct RequestedSignal {
    pub id: String,
    pub signal_name: String,
    pub claim_id: String,
}

/// Get all 'requested' signals for a workflow, ordered by creation time
///
/// Returns signal requests in FIFO order (oldest first) for proper matching.
pub async fn get_requested_signals<'e, E>(
    executor: E,
    workflow_id: &str,
) -> Result<Vec<RequestedSignal>>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let rows = sqlx::query(
        r#"
        SELECT id, signal_name, claim_id FROM signals
        WHERE workflow_id = $1 AND status = 'requested'
        ORDER BY created_at ASC
        "#,
    )
    .bind(workflow_id)
    .fetch_all(executor)
    .await
    .context("Failed to fetch requested signals")?;

    Ok(rows
        .into_iter()
        .map(|row| RequestedSignal {
            id: row.get::<sqlx::types::Uuid, _>("id").to_string(),
            signal_name: row.get("signal_name"),
            claim_id: row.get("claim_id"),
        })
        .collect())
}

/// Delete a signal by ID
pub async fn delete_signal<'e, E>(executor: E, signal_id: &str) -> Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query("DELETE FROM signals WHERE id = $1::uuid")
        .bind(signal_id)
        .execute(executor)
        .await
        .context("Failed to delete signal")?;

    Ok(())
}
