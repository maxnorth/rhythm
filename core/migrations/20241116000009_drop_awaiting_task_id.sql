-- Drop deprecated awaiting_task_id column
-- This column was replaced by storing the suspended task ID in locals.__suspended_task
-- The suspended task is now stored as part of the workflow's local variables,
-- which provides better consistency with the overall execution state.

ALTER TABLE workflow_execution_context
DROP COLUMN IF EXISTS awaiting_task_id;
