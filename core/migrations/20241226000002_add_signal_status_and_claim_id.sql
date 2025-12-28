-- Recreate signals table with status and claim_id for bidirectional matching
--
-- status: 'requested' (workflow waiting) or 'sent' (signal arrived)
-- claim_id: links a request to its claimed signal (NULL = unclaimed)

DROP TABLE IF EXISTS signals;

CREATE TABLE signals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id TEXT NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
    signal_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('requested', 'sent')),
    claim_id TEXT,
    payload JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for claim_id lookups (resolution check)
CREATE INDEX signals_claim_id ON signals (claim_id) WHERE claim_id IS NOT NULL;

-- Index for finding unclaimed sent signals (FIFO order)
CREATE INDEX signals_unclaimed ON signals (workflow_id, signal_name, created_at)
    WHERE status = 'sent' AND claim_id IS NULL;

-- Index for finding pending requests (FIFO order)
CREATE INDEX signals_requested ON signals (workflow_id, signal_name, created_at)
    WHERE status = 'requested';
