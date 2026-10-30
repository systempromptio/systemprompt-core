-- A proposal may carry the report of a dry-run replay of the brain pipeline
-- over a window of past events: what the facts would have become.
ALTER TABLE brain_proposals ADD COLUMN IF NOT EXISTS replay JSONB;
