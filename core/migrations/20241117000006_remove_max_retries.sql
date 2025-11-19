-- Remove max_retries column from executions table
-- Retry configuration will be handled globally via settings instead

ALTER TABLE executions
DROP COLUMN max_retries;
