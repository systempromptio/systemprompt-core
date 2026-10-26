-- The last explicit apply of each plane: the declared hash it applied, who ran
-- it, in which mode and when. `config_plane_base` and `config_conflicts` are
-- new tables and come from the declarative schema.
--
-- Why: lock footprint. ACCESS EXCLUSIVE on `config_plane_state` (one row per
-- plane) for a catalog-only change: nullable columns, no default, no rewrite.
-- `ai_requests` is not touched.
ALTER TABLE config_plane_state
    ADD COLUMN IF NOT EXISTS last_apply_declared_hash TEXT,
    ADD COLUMN IF NOT EXISTS last_apply_actor TEXT,
    ADD COLUMN IF NOT EXISTS last_apply_mode TEXT,
    ADD COLUMN IF NOT EXISTS last_apply_at TIMESTAMPTZ;
