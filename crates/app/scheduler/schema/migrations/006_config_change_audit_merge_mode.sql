-- An explicit apply (`config apply`, `/admin/sync`) records each entity it
-- wrote with mode `merge`, so the audit's mode check admits it beside the
-- three older modes. Widening only: no existing row is rewritten.
--
-- Why: lock footprint. DROP and ADD take ACCESS EXCLUSIVE on
-- `config_change_audit` for a catalog-only change; NOT VALID skips the scan
-- and VALIDATE then reads the table under SHARE UPDATE EXCLUSIVE, so writers
-- continue. `ai_requests` is not touched.
ALTER TABLE config_change_audit DROP CONSTRAINT IF EXISTS config_change_audit_mode_check;
ALTER TABLE config_change_audit
    ADD CONSTRAINT config_change_audit_mode_check
    CHECK (mode IN ('insert_only', 'overwrite', 'seed_if_empty', 'merge')) NOT VALID;
ALTER TABLE config_change_audit VALIDATE CONSTRAINT config_change_audit_mode_check;
