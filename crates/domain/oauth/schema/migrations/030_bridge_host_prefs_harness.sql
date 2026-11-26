-- Bridge host preferences name a host by its bridge id; `HostKind` gains
-- 'harness', so both preference tables admit it (schema/bridge_user_host_prefs.sql,
-- schema/bridge_user_host_model_prefs.sql). Each wider check is added NOT
-- VALID, validated under SHARE UPDATE EXCLUSIVE, and takes the narrower
-- check's name once that one is dropped.
ALTER TABLE bridge_user_host_prefs
    ADD CONSTRAINT bridge_user_host_prefs_host_id_widened
    CHECK (host_id IN (
        'claude-code', 'claude-desktop', 'codex', 'codex-cli', 'opencode', 'opencode-cli', 'hermes', 'harness')) NOT VALID;
ALTER TABLE bridge_user_host_prefs VALIDATE CONSTRAINT bridge_user_host_prefs_host_id_widened;
ALTER TABLE bridge_user_host_prefs DROP CONSTRAINT IF EXISTS bridge_user_host_prefs_host_id_check;
ALTER TABLE bridge_user_host_prefs
    RENAME CONSTRAINT bridge_user_host_prefs_host_id_widened TO bridge_user_host_prefs_host_id_check;

ALTER TABLE bridge_user_host_model_prefs
    ADD CONSTRAINT bridge_user_host_model_prefs_host_id_widened
    CHECK (host_id IN (
        'claude-code', 'claude-desktop', 'codex', 'codex-cli', 'opencode', 'opencode-cli', 'hermes', 'harness')) NOT VALID;
ALTER TABLE bridge_user_host_model_prefs VALIDATE CONSTRAINT bridge_user_host_model_prefs_host_id_widened;
ALTER TABLE bridge_user_host_model_prefs DROP CONSTRAINT IF EXISTS bridge_user_host_model_prefs_host_id_check;
ALTER TABLE bridge_user_host_model_prefs
    RENAME CONSTRAINT bridge_user_host_model_prefs_host_id_widened TO bridge_user_host_model_prefs_host_id_check;
