//! Signal outbox processing
//!
//! Handles matching and committing signal requests from the outbox.

use anyhow::Result;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};

use crate::db;
use crate::executor::Outbox;

/// Resolve pending signal claims for a workflow
///
/// Handles the race condition where both workflow and signal sender missed each other:
/// - Workflow inserted 'requested' row (waiting for signal)
/// - Signal sender inserted unclaimed 'sent' row (signal arrived)
///
/// For each 'requested' signal, if a matching unclaimed 'sent' signal exists:
/// 1. Claims the signal (sets claim_id on the 'sent' row)
/// 2. Deletes the 'requested' row
///
/// This is idempotent and safe to run multiple times. Should be called at the
/// start of workflow resumption, before the main execution loop.
///
/// Returns the number of signals that were resolved.
pub async fn resolve_signal_claims(pool: &PgPool, workflow_id: &str) -> Result<i64> {
    // Get all pending requests for this workflow
    let requests = db::signals::get_requested_signals(pool, workflow_id).await?;
    if requests.is_empty() {
        return Ok(0);
    }

    // Group requests by signal_name, preserving order
    let mut requests_by_name: HashMap<String, Vec<&db::signals::RequestedSignal>> = HashMap::new();
    for req in &requests {
        requests_by_name
            .entry(req.signal_name.clone())
            .or_default()
            .push(req);
    }

    // Match requests to unclaimed signals
    let mut matches: Vec<(&db::signals::RequestedSignal, String)> = Vec::new();

    for (signal_name, reqs) in &requests_by_name {
        // Get unclaimed signals for this name (in order)
        let available_signals = db::signals::get_unclaimed_signals_by_name(
            pool,
            workflow_id,
            signal_name,
            reqs.len() as i64,
        )
        .await?;

        // Match 1:1 in order
        for (req, signal_id) in reqs.iter().zip(available_signals.into_iter()) {
            matches.push((req, signal_id));
        }
    }

    if matches.is_empty() {
        return Ok(0);
    }

    // Commit matches in a transaction
    let mut tx = pool.begin().await?;

    for (req, signal_id) in &matches {
        // Claim the signal
        db::signals::claim_signal(&mut *tx, signal_id, &req.claim_id).await?;
        // Delete the request
        db::signals::delete_signal(&mut *tx, &req.id).await?;
    }

    tx.commit().await?;

    Ok(matches.len() as i64)
}

/// Match outbox signals to unclaimed DB signals (in-memory, no writes)
///
/// For each signal in the outbox without a signal_id, check if there's
/// an unclaimed 'sent' signal in the DB. If so, set the signal_id.
///
/// This function is idempotent - it can be called multiple times as new
/// signals are added to the outbox, and won't double-assign signal IDs.
pub async fn match_outbox_signals_to_unclaimed(
    pool: &PgPool,
    outbox: &mut Outbox,
    workflow_id: &str,
) -> Result<()> {
    // Collect signal IDs already assigned to outbox items (from previous calls)
    let already_claimed: HashSet<&str> = outbox
        .signals
        .iter()
        .filter_map(|s| s.signal_id.as_deref())
        .collect();

    // Count how many of each signal_name we need
    let mut needed: HashMap<String, usize> = HashMap::new();
    for signal in &outbox.signals {
        if signal.signal_id.is_none() {
            *needed.entry(signal.signal_name.clone()).or_default() += 1;
        }
    }

    if needed.is_empty() {
        return Ok(());
    }

    // For each signal_name, fetch candidates and filter out already-claimed IDs
    let mut available: HashMap<String, Vec<String>> = HashMap::new();
    for (name, count) in needed {
        // Fetch extra in case some are already claimed
        let ids = db::signals::get_unclaimed_signals_by_name(
            pool,
            workflow_id,
            &name,
            (count + already_claimed.len()) as i64,
        )
        .await?;
        let filtered: Vec<String> = ids
            .into_iter()
            .filter(|id| !already_claimed.contains(id.as_str()))
            .take(count)
            .collect();
        available.insert(name, filtered);
    }

    // Assign signals by taking from the front (oldest first)
    for outbox_signal in &mut outbox.signals {
        if outbox_signal.signal_id.is_some() {
            continue;
        }
        if let Some(ids) = available.get_mut(&outbox_signal.signal_name) {
            if !ids.is_empty() {
                outbox_signal.signal_id = Some(ids.remove(0));
            }
        }
    }

    Ok(())
}

/// Process signal outbox in transaction
///
/// 1. Claim matched signals (set claim_id on 'sent' signals)
/// 2. Insert requests for unmatched signals
///
/// Note: Race conditions where both workflow and signal sender miss each other are handled
/// by resolve_signal_claims at the start of workflow resumption.
pub async fn process_signal_outbox(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    outbox: &Outbox,
    workflow_id: &str,
) -> Result<()> {
    if outbox.signals.is_empty() {
        return Ok(());
    }

    for signal in &outbox.signals {
        if let Some(signal_id) = &signal.signal_id {
            // Matched during pre-loop - claim it
            db::signals::claim_signal(&mut **tx, signal_id, &signal.claim_id).await?;
        } else {
            // Not matched - insert request
            db::signals::insert_signal_request(
                &mut **tx,
                workflow_id,
                &signal.signal_name,
                &signal.claim_id,
            )
            .await?;
        }
    }

    Ok(())
}
