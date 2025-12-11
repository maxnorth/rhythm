//! Workflow Definitions Database Operations

use anyhow::{Context, Result};
use sqlx::{PgPool, Row};

/// Get existing workflow definition by name and version hash
///
/// Returns the workflow ID if it exists, None otherwise.
pub async fn get_workflow_by_name_and_hash(
    pool: &PgPool,
    name: &str,
    version_hash: &str,
) -> Result<Option<i32>> {
    let row = sqlx::query(
        r#"
        SELECT id
        FROM workflow_definitions
        WHERE name = $1 AND version_hash = $2
        LIMIT 1
        "#,
    )
    .bind(name)
    .bind(version_hash)
    .fetch_optional(pool)
    .await
    .context("Failed to check for existing workflow definition")?;

    Ok(row.map(|r| r.get("id")))
}

/// Create a new workflow definition
///
/// Inserts a workflow definition with the given name, version hash, and source code.
/// Returns the workflow definition ID.
pub async fn create_workflow_definition(
    pool: &PgPool,
    name: &str,
    version_hash: &str,
    source: &str,
) -> Result<i32> {
    let row = sqlx::query(
        r#"
        INSERT INTO workflow_definitions (name, version_hash, source, parsed_steps, file_path)
        VALUES ($1, $2, $3, '{}', '')
        RETURNING id
        "#,
    )
    .bind(name)
    .bind(version_hash)
    .bind(source)
    .fetch_one(pool)
    .await
    .context("Failed to create workflow definition")?;

    Ok(row.get("id"))
}

/// Get workflow source from workflow_definitions by name
///
/// Returns the workflow definition ID and source code for the most recently
/// created workflow with the given name.
pub async fn get_workflow_by_name(pool: &PgPool, workflow_name: &str) -> Result<(i32, String)> {
    let row = sqlx::query(
        r#"
        SELECT id, source
        FROM workflow_definitions
        WHERE name = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(workflow_name)
    .fetch_one(pool)
    .await
    .context("Failed to fetch workflow definition")?;

    Ok((row.get("id"), row.get("source")))
}
