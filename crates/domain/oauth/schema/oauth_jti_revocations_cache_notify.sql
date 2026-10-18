-- Cross-replica cache invalidation: a committed revocation notifies every
-- replica's event bridge, which drops its cached not-revoked answers.
CREATE OR REPLACE FUNCTION sp_notify_jti_revocation_cache() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_notify('systemprompt_cache', 'jti_revocation');
    RETURN NULL;
END;
$$;
CREATE OR REPLACE TRIGGER oauth_jti_revocations_cache_notify
AFTER INSERT ON oauth_jti_revocations
FOR EACH STATEMENT EXECUTE FUNCTION sp_notify_jti_revocation_cache();
