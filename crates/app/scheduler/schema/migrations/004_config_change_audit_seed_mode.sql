-- A boot seed is recorded with mode `seed_if_empty` (actor `seed`), so the
-- audit's mode check admits it alongside `insert_only` and `overwrite`.
-- Widening only: a binary that writes the two older modes is unaffected.
ALTER TABLE config_change_audit DROP CONSTRAINT IF EXISTS config_change_audit_mode_check;
ALTER TABLE config_change_audit
    ADD CONSTRAINT config_change_audit_mode_check
    CHECK (mode IN ('insert_only', 'overwrite', 'seed_if_empty'));
