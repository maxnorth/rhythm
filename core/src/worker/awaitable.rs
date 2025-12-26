//! Awaitable resolution logic
//!
//! Recursively resolves awaitables (Task, Timer, All, Any, Race) to determine
//! if they're ready and what value to resume with.

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashMap;

use crate::db;
use crate::executor::{errors::ErrorInfo, json_to_val, Awaitable, Val};
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
/// Uses `Box::pin` for async recursion.
pub fn resolve_awaitable<'a>(
    pool: &'a PgPool,
    awaitable: &'a Awaitable,
    db_now: DateTime<Utc>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<AwaitableStatus>> + Send + 'a>> {
    Box::pin(async move {
        match awaitable {
            Awaitable::Task(task_id) => resolve_task(pool, task_id).await,
            Awaitable::Timer { fire_at } => Ok(resolve_timer(*fire_at, db_now)),
            Awaitable::All { items, is_object } => {
                resolve_all(pool, items, *is_object, db_now).await
            }
            Awaitable::Any { items, is_object } => {
                resolve_any(pool, items, *is_object, db_now).await
            }
            Awaitable::Race { items, is_object } => {
                resolve_race(pool, items, *is_object, db_now).await
            }
        }
    })
}

async fn resolve_task(pool: &PgPool, task_id: &str) -> Result<AwaitableStatus> {
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
        // Task not in DB yet (probably in outbox, will be saved soon)
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
) -> Result<AwaitableStatus> {
    let mut results: Vec<(String, Val)> = Vec::new();

    for (key, awaitable) in items {
        match resolve_awaitable(pool, awaitable, db_now).await? {
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
    db_now: DateTime<Utc>,
) -> Result<AwaitableStatus> {
    let mut has_pending = false;

    for (key, awaitable) in items {
        match resolve_awaitable(pool, awaitable, db_now).await? {
            AwaitableStatus::Success(val) => {
                // First success - return { key, value }
                let result = build_winner_result(key, val, is_object);
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
    db_now: DateTime<Utc>,
) -> Result<AwaitableStatus> {
    for (key, awaitable) in items {
        match resolve_awaitable(pool, awaitable, db_now).await? {
            AwaitableStatus::Success(val) => {
                // First settled (success)
                let result = build_winner_result(key, val, is_object);
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
