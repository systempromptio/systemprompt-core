-- @no-transaction
-- A plane whose declared requirement the host does not provide is recorded
-- as `unavailable` instead of being read.
-- The constraint is added NOT VALID outside a transaction, so its ACCESS
-- EXCLUSIVE lock is released before VALIDATE scans the rows under SHARE
-- UPDATE EXCLUSIVE, which does not block writers.
ALTER TABLE config_plane_state DROP CONSTRAINT IF EXISTS config_plane_state_outcome_check;
ALTER TABLE config_plane_state ADD CONSTRAINT config_plane_state_outcome_check
    CHECK (outcome IN ('in_sync', 'corrected', 'seeded', 'drift_reported', 'undeclared', 'unavailable', 'failed')) NOT VALID;
ALTER TABLE config_plane_state VALIDATE CONSTRAINT config_plane_state_outcome_check;
