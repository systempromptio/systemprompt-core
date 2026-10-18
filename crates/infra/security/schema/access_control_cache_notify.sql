-- Cross-replica cache invalidation: a committed write to the access-control
-- rules or entities notifies every replica's event bridge, which reloads its
-- authz chain index on the next decision.
CREATE OR REPLACE FUNCTION sp_notify_authz_cache() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_notify('systemprompt_cache', 'authz');
    RETURN NULL;
END;
$$;
CREATE OR REPLACE TRIGGER access_control_rules_cache_notify
AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON access_control_rules
FOR EACH STATEMENT EXECUTE FUNCTION sp_notify_authz_cache();
CREATE OR REPLACE TRIGGER access_control_entities_cache_notify
AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON access_control_entities
FOR EACH STATEMENT EXECUTE FUNCTION sp_notify_authz_cache();
