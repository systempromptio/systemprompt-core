-- Record the memory recall applied to a gateway request, if any.
--
-- The gateway can inject recalled memories into the request before forwarding
-- upstream. This column captures a one-line descriptor of that decision (e.g.
-- `injected=3 withheld=2 tokens=412 as_of=2026-10-30T10:00:00Z` or
-- `skipped=deadline`) so an audit shows it. NULL when recall did not run and
-- for non-gateway requests. Nullable with no backfill.

ALTER TABLE ai_requests
    ADD COLUMN IF NOT EXISTS memory_recall TEXT;
