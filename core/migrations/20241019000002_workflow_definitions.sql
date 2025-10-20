-- Workflow definitions table for DSL-based workflows

-- Workflow definitions (registered at initialization)
CREATE TABLE IF NOT EXISTS workflow_definitions (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    version_hash TEXT NOT NULL,
    source TEXT NOT NULL,
    parsed_steps JSONB NOT NULL,
    file_path TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- Ensure unique workflow name + version combinations
    UNIQUE(name, version_hash)
);

-- Index for looking up workflows by name (will get latest by created_at)
CREATE INDEX IF NOT EXISTS idx_workflow_definitions_name
    ON workflow_definitions(name, created_at DESC);

-- Index for looking up specific versions
CREATE INDEX IF NOT EXISTS idx_workflow_definitions_version
    ON workflow_definitions(version_hash);

-- Insert schema version
INSERT INTO schema_migrations (version) VALUES (2) ON CONFLICT DO NOTHING;
