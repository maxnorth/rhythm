-- Workflows database schema

-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Executions table (unified for tasks and workflows)
CREATE TABLE IF NOT EXISTS executions (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL CHECK (type IN ('task', 'workflow')),
    function_name TEXT NOT NULL,
    queue TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'suspended', 'completed', 'failed')),
    priority INTEGER NOT NULL DEFAULT 5,

    -- Execution metadata
    args JSONB NOT NULL DEFAULT '[]',
    kwargs JSONB NOT NULL DEFAULT '{}',
    options JSONB NOT NULL DEFAULT '{}',

    -- Results and errors
    result JSONB,
    error JSONB,

    -- Retry tracking
    attempt INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,

    -- Workflow-specific fields
    parent_workflow_id TEXT REFERENCES executions(id) ON DELETE CASCADE,
    checkpoint JSONB,

    -- Timing
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    claimed_at TIMESTAMP WITH TIME ZONE,
    completed_at TIMESTAMP WITH TIME ZONE,
    timeout_seconds INTEGER,

    -- Worker assignment
    worker_id TEXT
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_executions_queue_status_priority
    ON executions(queue, status, priority DESC, created_at ASC)
    WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS idx_executions_worker_status
    ON executions(worker_id, status)
    WHERE status IN ('running', 'suspended');

CREATE INDEX IF NOT EXISTS idx_executions_parent
    ON executions(parent_workflow_id)
    WHERE parent_workflow_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_executions_created_at
    ON executions(created_at DESC);

-- Worker heartbeats
CREATE TABLE IF NOT EXISTS worker_heartbeats (
    worker_id TEXT PRIMARY KEY,
    last_heartbeat TIMESTAMP WITH TIME ZONE NOT NULL,
    queues TEXT[] NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('running', 'stopped')),
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_worker_heartbeats_last_heartbeat
    ON worker_heartbeats(last_heartbeat DESC);

-- Signals for workflow communication
CREATE TABLE IF NOT EXISTS workflow_signals (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
    signal_name TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    consumed BOOLEAN DEFAULT FALSE
);

CREATE INDEX IF NOT EXISTS idx_workflow_signals_workflow_name
    ON workflow_signals(workflow_id, signal_name, consumed);

-- Dead letter queue (failed executions)
CREATE TABLE IF NOT EXISTS dead_letter_queue (
    id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL,
    execution_data JSONB NOT NULL,
    failure_reason TEXT,
    failed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_dlq_failed_at
    ON dead_letter_queue(failed_at DESC);

-- Insert initial schema version
INSERT INTO schema_migrations (version) VALUES (1) ON CONFLICT DO NOTHING;
