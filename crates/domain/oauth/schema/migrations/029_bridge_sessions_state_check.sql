-- bridge_sessions.state is the BridgeState vocabulary or unreported (NULL);
-- the heartbeat writer binds BridgeState's own spelling, so no rewrite
-- precedes the constraint.
ALTER TABLE bridge_sessions DROP CONSTRAINT IF EXISTS bridge_sessions_state_check;
ALTER TABLE bridge_sessions ADD CONSTRAINT bridge_sessions_state_check
    CHECK (state IN ('active', 'disabled'));
