-- Change ast_path from TEXT to JSONB
-- The code expects ast_path to be JSONB (JSON array format like [0] or [1, "then_statements", 0])
-- but the original migration created it as TEXT

-- First, drop the default
ALTER TABLE workflow_execution_context
  ALTER COLUMN ast_path DROP DEFAULT;

-- Change type from TEXT to JSONB, converting existing data
ALTER TABLE workflow_execution_context
  ALTER COLUMN ast_path TYPE JSONB USING ast_path::jsonb;

-- Restore default as JSONB
ALTER TABLE workflow_execution_context
  ALTER COLUMN ast_path SET DEFAULT '[0]'::jsonb;
