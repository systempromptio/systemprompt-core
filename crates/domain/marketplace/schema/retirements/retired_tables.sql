-- Tables this member's migration chain has retired (migration 016). They are
-- declared, not executed: an established database may still hold one until
-- the consumer whose view reads it drops it.
DROP TABLE IF EXISTS managed_invocation_attributions;
DROP TABLE IF EXISTS managed_consumer_attribution_history;
DROP TABLE IF EXISTS managed_consumer_attribution_projection;
DROP TABLE IF EXISTS managed_consumer_invocation_evidence;
DROP TABLE IF EXISTS managed_api_operations;
