use anyhow::{anyhow, Context, Result};
use sqlx::PgPool;

use crate::application::WorkflowFile;
use crate::db;

/// Service for initialization operations (migrations, workflow registration, etc.)
#[derive(Clone)]
pub struct InitializationService {
    pool: PgPool,
}

impl InitializationService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Run initialization tasks (migrations, workflow registration)
    pub async fn initialize(&self, auto_migrate: bool, workflows: Vec<WorkflowFile>) -> Result<()> {
        // Run migrations if requested
        if auto_migrate {
            self.run_migrations()
                .await
                .context("Failed to run automatic migrations")?;
        }

        // Register workflows if provided
        if !workflows.is_empty() {
            self.register_workflows(workflows)
                .await
                .context("Failed to register workflows")?;
        }

        Ok(())
    }

    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<()> {
        db::migrate(&self.pool).await
    }

    /// Register workflows in the database (idempotent)
    pub async fn register_workflows(&self, workflows: Vec<WorkflowFile>) -> Result<()> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        for workflow in workflows {
            // Parse and validate the workflow source
            let _ast = crate::parser::parse(&workflow.source).map_err(|e| {
                anyhow!(
                    "Failed to parse workflow '{}' from {}: {:?}",
                    workflow.name,
                    workflow.file_path,
                    e
                )
            })?;

            // Generate version hash
            let mut hasher = DefaultHasher::new();
            workflow.source.hash(&mut hasher);
            let version_hash = format!("{:x}", hasher.finish());

            // Check if workflow already exists
            let existing_id = db::workflow_definitions::get_workflow_by_name_and_hash(
                &self.pool,
                &workflow.name,
                &version_hash,
            )
            .await
            .with_context(|| {
                format!("Failed to check for existing workflow '{}'", workflow.name)
            })?;

            if existing_id.is_some() {
                // Workflow already registered, skip
                continue;
            }

            // Register the new workflow definition
            db::workflow_definitions::create_workflow_definition(
                &self.pool,
                &workflow.name,
                &version_hash,
                &workflow.source,
            )
            .await
            .with_context(|| format!("Failed to register workflow '{}'", workflow.name))?;
        }
        Ok(())
    }
}
