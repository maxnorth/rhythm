-- Create signals table for workflow human-in-the-loop patterns
--
-- Signals allow external systems to send data to waiting workflows.
-- Each signal is associated with a workflow and a named channel.
-- Workflows consume signals via Signal.next(name) which returns the latest
-- unconsumed signal for that channel.

CREATE TABLE signals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id TEXT NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
    signal_name TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for signal lookup: find signals for a workflow/channel, ordered by time
-- Used by get_latest_signal_after to find the next signal after a cursor
CREATE INDEX idx_signals_lookup
ON signals(workflow_id, signal_name, created_at DESC);
