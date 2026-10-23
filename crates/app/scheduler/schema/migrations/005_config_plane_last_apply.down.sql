ALTER TABLE config_plane_state
    DROP COLUMN IF EXISTS last_apply_at,
    DROP COLUMN IF EXISTS last_apply_mode,
    DROP COLUMN IF EXISTS last_apply_actor,
    DROP COLUMN IF EXISTS last_apply_declared_hash;
