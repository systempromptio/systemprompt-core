-- Provenance and invalidation for config plane tables.
--
-- A writer sets three transaction-local settings before it writes
-- (`with_change_context`): `systemprompt.actor`, `systemprompt.reason` and
-- `systemprompt.source`. `sp_stamp_change()` is a BEFORE row trigger that
-- copies them into `changed_by`, `reason` and `source` and stamps
-- `changed_at`. An unset actor or reason records NULL, so a write that skipped
-- the helper is visible as unattributed; an unset source keeps the row's own.
-- It reads settings only, never another table.
--
-- `sp_bump_config_version()` is an AFTER statement trigger whose single
-- argument names the plane. It bumps the one `config_version` row and sends
-- `config:<plane>` on `systemprompt_cache`, which every node's listener turns
-- into a cache epoch. The row update serialises concurrent plane writers until
-- they commit, so versions are strictly increasing in commit order.
CREATE OR REPLACE FUNCTION sp_stamp_change() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    NEW.changed_by := NULLIF(current_setting('systemprompt.actor', true), '');
    NEW.reason := NULLIF(current_setting('systemprompt.reason', true), '');
    NEW.source := COALESCE(NULLIF(current_setting('systemprompt.source', true), ''), NEW.source);
    NEW.changed_at := NOW();
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION sp_bump_config_version() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO config_version (id, version, plane, bumped_at)
    VALUES (1, 1, TG_ARGV[0], NOW())
    ON CONFLICT (id) DO UPDATE
        SET version = config_version.version + 1,
            plane = EXCLUDED.plane,
            bumped_at = EXCLUDED.bumped_at;
    PERFORM pg_notify('systemprompt_cache', 'config:' || TG_ARGV[0]);
    RETURN NULL;
END;
$$;
