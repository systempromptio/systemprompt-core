-- The reporting baseline rebuild is no longer one transaction: it fences a
-- cutoff, then writes each source in committed pages, then flips
-- `initialized`. These columns are the in-progress marker: a node that finds
-- a fresh heartbeat leaves the running rebuild alone, a stale one is taken
-- over, and `analytics projection status` shows where the rebuild is.

ALTER TABLE analytics_projection_state ADD COLUMN IF NOT EXISTS rebuild_started_at TIMESTAMPTZ;
ALTER TABLE analytics_projection_state ADD COLUMN IF NOT EXISTS rebuild_heartbeat_at TIMESTAMPTZ;
ALTER TABLE analytics_projection_state ADD COLUMN IF NOT EXISTS rebuild_source TEXT;
ALTER TABLE analytics_projection_state ADD COLUMN IF NOT EXISTS rebuild_rows BIGINT NOT NULL DEFAULT 0;
