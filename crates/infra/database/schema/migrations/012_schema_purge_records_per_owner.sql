-- Each member declares the user purge of its own tables. Records written
-- while the users member declared the purges of the events, logging, authz,
-- oauth, mcp, ai, agent and files tables are removed: every one of these
-- members admitted by this install records its own declarations again, and
-- one left out is reported by the user purge as unrecorded instead of being
-- covered by a stale set.
DELETE FROM schema_purge_records
WHERE extension_id IN (
    'users', 'events', 'logging', 'authz', 'oauth', 'mcp', 'ai', 'agent', 'files'
);
