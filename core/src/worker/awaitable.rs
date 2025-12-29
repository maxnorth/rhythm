//! Awaitable resolution logic
//!
//! Recursively resolves awaitables (Task, Timer, All, Any, Race, Signal) to determine
//! if they're ready and what value to resume with.

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashMap;

use crate::db;
use crate::executor::{errors::ErrorInfo, json_to_val, Awaitable, Outbox, Val};
use crate::types::ExecutionStatus;

/// Result of checking an awaitable's status
pub enum AwaitableStatus {
    /// Awaitable is not ready yet
    Pending,
    /// Awaitable completed successfully with a value
    Success(Val),
    /// Awaitable failed with an error value
    Error(Val),
}

/// Recursively resolve an awaitable to check if it's ready.
///
/// Returns the status: Pending if not ready, Success/Error if ready with a value.
/// Handles nested composites by recursively resolving inner awaitables.
///
/// `signal_outbox` contains signals created in the current execution run.
/// Signals not in the outbox are from previous runs and checked via DB.
///
/// Uses `Box::pin` for async recursion.
pub fn resolve_awaitable<'a>(
    pool: &'a PgPool,
    awaitable: &'a Awaitable,
    db_now: DateTime<Utc>,
    outbox: &'a Outbox,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<AwaitableStatus>> + Send + 'a>> {
    Box::pin(async move {
        match awaitable {
            Awaitable::Task(task_id) => resolve_task(pool, task_id, outbox).await,
            Awaitable::Timer { fire_at } => Ok(resolve_timer(*fire_at, db_now)),
            Awaitable::All { items, is_object } => {
                resolve_all(pool, items, *is_object, db_now, outbox).await
            }
            Awaitable::Any {
                items,
                is_object,
                with_kv,
            } => resolve_any(pool, items, *is_object, *with_kv, db_now, outbox).await,
            Awaitable::Race {
                items,
                is_object,
                with_kv,
            } => resolve_race(pool, items, *is_object, *with_kv, db_now, outbox).await,
            Awaitable::Signal { name: _, claim_id } => resolve_signal(pool, claim_id, outbox).await,
        }
    })
}

/// Resolve a signal awaitable
///
/// Resolution logic:
/// 1. If claim_id is in signal outbox with signal_id → fetch payload, Ready
/// 2. If claim_id is in signal outbox without signal_id → Pending
/// 3. If claim_id not in outbox → check DB by claim_id
async fn resolve_signal(pool: &PgPool, claim_id: &str, outbox: &Outbox) -> Result<AwaitableStatus> {
    // Check if this signal is in the outbox (created this run)
    if let Some(outbox_signal) = outbox.get_signal(claim_id) {
        if let Some(signal_id) = &outbox_signal.signal_id {
            // Matched to an unclaimed signal - fetch payload
            let payload = db::signals::get_signal_payload(pool, signal_id).await?;
            let val = json_to_val(&payload)?;
            return Ok(AwaitableStatus::Success(val));
        } else {
            // In outbox but not matched yet - pending
            return Ok(AwaitableStatus::Pending);
        }
    }

    // Not in outbox - check DB (signal from previous run)
    if let Some(payload) = db::signals::check_signal_claimed(pool, claim_id).await? {
        let val = json_to_val(&payload)?;
        Ok(AwaitableStatus::Success(val))
    } else {
        Ok(AwaitableStatus::Pending)
    }
}

async fn resolve_task(pool: &PgPool, task_id: &str, outbox: &Outbox) -> Result<AwaitableStatus> {
    // If task is in outbox, it was just created this run - skip DB query
    if outbox.has_task(task_id) {
        return Ok(AwaitableStatus::Pending);
    }

    if let Some(task_execution) = db::executions::get_execution(pool, task_id).await? {
        match task_execution.status {
            ExecutionStatus::Completed => {
                let result = task_execution
                    .output
                    .map(|json| json_to_val(&json))
                    .transpose()?
                    .unwrap_or(Val::Null);
                Ok(AwaitableStatus::Success(result))
            }
            ExecutionStatus::Failed => {
                let result = task_execution
                    .output
                    .map(|json| json_to_val(&json))
                    .transpose()?
                    .unwrap_or(Val::Null);
                Ok(AwaitableStatus::Error(result))
            }
            _ => Ok(AwaitableStatus::Pending),
        }
    } else {
        // Task not in DB yet
        Ok(AwaitableStatus::Pending)
    }
}

fn resolve_timer(fire_at: DateTime<Utc>, db_now: DateTime<Utc>) -> AwaitableStatus {
    if fire_at <= db_now {
        AwaitableStatus::Success(Val::Null)
    } else {
        AwaitableStatus::Pending
    }
}

/// Promise.all - wait for all to complete, fail fast on error
async fn resolve_all(
    pool: &PgPool,
    items: &[(String, Awaitable)],
    is_object: bool,
    db_now: DateTime<Utc>,
    outbox: &Outbox,
) -> Result<AwaitableStatus> {
    let mut results: Vec<(String, Val)> = Vec::new();

    for (key, awaitable) in items {
        match resolve_awaitable(pool, awaitable, db_now, outbox).await? {
            AwaitableStatus::Success(val) => {
                results.push((key.clone(), val));
            }
            AwaitableStatus::Error(err) => {
                // Fail fast - return error immediately
                return Ok(AwaitableStatus::Error(err));
            }
            AwaitableStatus::Pending => {
                // At least one pending - whole thing is pending
                return Ok(AwaitableStatus::Pending);
            }
        }
    }

    // All completed successfully - build result
    let result = if is_object {
        let obj: HashMap<String, Val> = results.into_iter().collect();
        Val::Obj(obj)
    } else {
        // Items are already in order from iteration
        Val::List(results.into_iter().map(|(_, v)| v).collect())
    };

    Ok(AwaitableStatus::Success(result))
}

/// Promise.any - wait for first success, fail only if all fail
async fn resolve_any(
    pool: &PgPool,
    items: &[(String, Awaitable)],
    is_object: bool,
    with_kv: bool,
    db_now: DateTime<Utc>,
    outbox: &Outbox,
) -> Result<AwaitableStatus> {
    let mut has_pending = false;

    for (key, awaitable) in items {
        match resolve_awaitable(pool, awaitable, db_now, outbox).await? {
            AwaitableStatus::Success(val) => {
                // First success - return value or { key, value } based on with_kv flag
                let result = if with_kv {
                    build_winner_result(key, val, is_object)
                } else {
                    val
                };
                return Ok(AwaitableStatus::Success(result));
            }
            AwaitableStatus::Error(_) => {
                // Continue checking others
            }
            AwaitableStatus::Pending => {
                has_pending = true;
            }
        }
    }

    if has_pending {
        // Some still pending, no success yet
        Ok(AwaitableStatus::Pending)
    } else {
        // All failed - return AggregateError
        let aggregate_error = Val::Error(ErrorInfo::new("AggregateError", "All promises rejected"));
        Ok(AwaitableStatus::Error(aggregate_error))
    }
}

/// Promise.race - wait for first to settle (success or error)
async fn resolve_race(
    pool: &PgPool,
    items: &[(String, Awaitable)],
    is_object: bool,
    with_kv: bool,
    db_now: DateTime<Utc>,
    outbox: &Outbox,
) -> Result<AwaitableStatus> {
    for (key, awaitable) in items {
        match resolve_awaitable(pool, awaitable, db_now, outbox).await? {
            AwaitableStatus::Success(val) => {
                // First settled (success) - return value or { key, value } based on with_kv flag
                let result = if with_kv {
                    build_winner_result(key, val, is_object)
                } else {
                    val
                };
                return Ok(AwaitableStatus::Success(result));
            }
            AwaitableStatus::Error(err) => {
                // First settled (error) - race propagates the error
                return Ok(AwaitableStatus::Error(err));
            }
            AwaitableStatus::Pending => {
                // Keep checking others
            }
        }
    }

    // All still pending
    Ok(AwaitableStatus::Pending)
}

/// Build the { key, value } result object for race/any winners
fn build_winner_result(key: &str, value: Val, is_object: bool) -> Val {
    let mut result = HashMap::new();
    if is_object {
        result.insert("key".to_string(), Val::Str(key.to_string()));
    } else {
        // For array form, key is a numeric string - convert to number
        let key_val = key
            .parse::<f64>()
            .map(Val::Num)
            .unwrap_or_else(|_| Val::Str(key.to_string()));
        result.insert("key".to_string(), key_val);
    }
    result.insert("value".to_string(), value);
    Val::Obj(result)
}
