-- Workflow execution context table for DSL-based workflow executions

-- Stores the execution state for each workflow instance
CREATE TABLE IF NOT EXISTS workflow_execution_context (
    execution_id TEXT PRIMARY KEY REFERENCES executions(id) ON DELETE CASCADE,
    workflow_definition_id INTEGER NOT NULL REFERENCES workflow_definitions(id),

    -- Current execution state
    statement_index INTEGER NOT NULL DEFAULT 0,
    locals JSONB NOT NULL DEFAULT '{}',
    awaiting_task_id TEXT,

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Index for looking up by workflow definition
CREATE INDEX IF NOT EXISTS idx_workflow_execution_context_definition
    ON workflow_execution_context(workflow_definition_id);

-- Insert schema version
INSERT INTO schema_migrations (version) VALUES (3) ON CONFLICT DO NOTHING;
