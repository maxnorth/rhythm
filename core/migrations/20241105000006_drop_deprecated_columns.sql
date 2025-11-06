-- Drop deprecated statement_index column
-- statement_index: replaced by ast_path (no longer needed)
--
-- NOTE: awaiting_task_id will be dropped in a future migration after
-- Phase 5 refactoring implements __suspended_task in locals

ALTER TABLE workflow_execution_context
DROP COLUMN IF EXISTS statement_index;
