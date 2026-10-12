-- Installation coverage becomes a drained projection
-- (schema/managed_installation_coverage_queue.sql): statement triggers queue
-- the owners whose receipts, grants, publication selections or consumer
-- devices changed, and the `managed_inventory_refresh` job drains them
-- through `refresh_managed_installation_coverage` instead of rebuilding every
-- owner on every tick. Every owner is queued once here so the first drain
-- rebuilds what the old unconditional pass would have; keys only.
INSERT INTO projection_backlog (projection, key)
SELECT 'managed_installation_coverage', owner_id FROM managed_resources
ON CONFLICT DO NOTHING;
