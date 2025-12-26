-- Create scheduled queue table
--
-- Generic "run this later" primitive for the workflow engine.
-- Supports workflow resume timers, cron jobs, one-off scheduled tasks, etc.

CREATE TABLE scheduled_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_at TIMESTAMP NOT NULL,
    params JSONB NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Index for promotion query: find items ready to run
CREATE INDEX idx_scheduled_queue_run_at
ON scheduled_queue(run_at ASC);
