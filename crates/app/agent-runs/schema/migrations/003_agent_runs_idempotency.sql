-- An API caller names each run with an idempotency key, unique per origin;
-- the request digest tells a retry of the same request from a reuse of the
-- key for a different one.
ALTER TABLE agent_runs ADD COLUMN IF NOT EXISTS idempotency_key TEXT;
ALTER TABLE agent_runs ADD COLUMN IF NOT EXISTS request_digest TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_runs_idempotency
    ON agent_runs(origin_kind, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
