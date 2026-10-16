-- Cross-replica cache invalidation: a committed write to the gateway policy
-- rows notifies every replica's event bridge, which drops its cached policy.
CREATE OR REPLACE FUNCTION sp_notify_gateway_policy_cache() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_notify('systemprompt_cache', 'gateway_policy');
    RETURN NULL;
END;
$$;
CREATE OR REPLACE TRIGGER ai_gateway_policies_cache_notify
AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON ai_gateway_policies
FOR EACH STATEMENT EXECUTE FUNCTION sp_notify_gateway_policy_cache();
