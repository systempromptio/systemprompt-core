-- Bridge host preferences name a host by its bridge id; the id set is the
-- `HostKind` vocabulary, so both preference tables are constrained to it. A
-- row naming an id outside the set selected no host and is dropped first.
DELETE FROM bridge_user_host_prefs
WHERE host_id NOT IN ('claude-code', 'claude-desktop', 'codex', 'codex-cli', 'opencode', 'opencode-cli', 'hermes');

DELETE FROM bridge_user_host_model_prefs
WHERE host_id NOT IN ('claude-code', 'claude-desktop', 'codex', 'codex-cli', 'opencode', 'opencode-cli', 'hermes');

ALTER TABLE bridge_user_host_prefs
    DROP CONSTRAINT IF EXISTS bridge_user_host_prefs_host_id_check;
ALTER TABLE bridge_user_host_prefs
    ADD CONSTRAINT bridge_user_host_prefs_host_id_check
    CHECK (host_id IN ('claude-code', 'claude-desktop', 'codex', 'codex-cli', 'opencode', 'opencode-cli', 'hermes'));

ALTER TABLE bridge_user_host_model_prefs
    DROP CONSTRAINT IF EXISTS bridge_user_host_model_prefs_host_id_check;
ALTER TABLE bridge_user_host_model_prefs
    ADD CONSTRAINT bridge_user_host_model_prefs_host_id_check
    CHECK (host_id IN ('claude-code', 'claude-desktop', 'codex', 'codex-cli', 'opencode', 'opencode-cli', 'hermes'));
