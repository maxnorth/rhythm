-- Drop unused checkpoint column from executions table
-- This column was never used in production - workflow state is stored
-- in workflow_execution_context table instead

ALTER TABLE executions DROP COLUMN IF EXISTS checkpoint;
