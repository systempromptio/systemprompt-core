-- @no-transaction
-- bridge_sessions.state is the BridgeState vocabulary or unreported (NULL);
-- the heartbeat writer binds BridgeState's own spelling, so no rewrite
-- precedes the constraint.
-- The constraint is added NOT VALID outside a transaction, so its ACCESS
-- EXCLUSIVE lock is released before VALIDATE scans the rows under SHARE
-- UPDATE EXCLUSIVE, which does not block writers.
ALTER TABLE bridge_sessions DROP CONSTRAINT IF EXISTS bridge_sessions_state_check;
ALTER TABLE bridge_sessions ADD CONSTRAINT bridge_sessions_state_check
    CHECK (state IN ('active', 'disabled')) NOT VALID;
ALTER TABLE bridge_sessions VALIDATE CONSTRAINT bridge_sessions_state_check;
