-- Merge result and error columns into a single output column
-- The status field determines what the output represents:
--   status = 'completed' -> output is the result
--   status = 'failed' -> output is the error

-- Add new output column
ALTER TABLE executions
ADD COLUMN output jsonb;

-- Migrate existing data: use result if present, otherwise use error
UPDATE executions
SET output = COALESCE(result, error);

-- Drop old columns
ALTER TABLE executions
DROP COLUMN result,
DROP COLUMN error;
