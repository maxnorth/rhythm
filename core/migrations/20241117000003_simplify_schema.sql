-- Simplify schema by removing unnecessary complexity

-- Drop dead letter queue table
DROP TABLE IF EXISTS dead_letter_queue;

-- Remove complexity from executions table
ALTER TABLE executions
DROP COLUMN IF EXISTS worker_id,
DROP COLUMN IF EXISTS claimed_at,
DROP COLUMN IF EXISTS priority,
DROP COLUMN IF EXISTS options,
DROP COLUMN IF EXISTS timeout_seconds;

-- Drop indexes that are no longer needed
DROP INDEX IF EXISTS idx_executions_worker_id;
DROP INDEX IF EXISTS idx_executions_priority;
