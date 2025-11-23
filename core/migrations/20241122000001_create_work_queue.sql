-- Create work queue table for V2 scheduler
--
-- This table provides the scheduling primitive for the V2 workflow engine.
-- It's ephemeral (hot table) separate from the historical executions table.

CREATE TABLE work_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    execution_id TEXT NOT NULL,
    queue TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    claimed_until TIMESTAMP DEFAULT NULL
);

-- Index for completion/lookup by execution_id
CREATE INDEX idx_work_queue_execution_id
ON work_queue(execution_id);

-- Dual-row enforcement: allows one claimed + one unclaimed per execution_id
-- Using expression-based unique index to consolidate what would be two partial indexes
CREATE UNIQUE INDEX idx_work_queue_execution_claimed_state
ON work_queue(execution_id, (claimed_until IS NULL));

-- Index for efficient claiming
-- Optimized for query: WHERE queue = ? AND (claimed_until IS NULL OR claimed_until < NOW())
-- Sorted by: priority DESC, created_at ASC
CREATE INDEX idx_work_queue_claim
ON work_queue(queue, claimed_until, priority DESC, created_at ASC);
