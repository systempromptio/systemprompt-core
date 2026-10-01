-- Host preferences are now decoded into the closed HostKind set, and a row
-- outside it is a decode error. Earlier releases accepted host ids that no
-- longer exist (`cowork`), so such rows are removed: a preference for a host
-- the bridge cannot run selects nothing.
DELETE FROM bridge_user_host_prefs
WHERE host_id NOT IN ('claude-code', 'claude-desktop', 'codex-cli', 'hermes', 'opencode');

DELETE FROM bridge_user_host_model_prefs
WHERE host_id NOT IN ('claude-code', 'claude-desktop', 'codex-cli', 'hermes', 'opencode');
