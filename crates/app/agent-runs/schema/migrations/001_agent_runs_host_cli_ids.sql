-- A headless run names the CLI packaging of its host. The OpenCode CLI's id is
-- `opencode-cli`; rows queued under the product id move to it before the
-- column is constrained to the headless host set.
UPDATE agent_runs SET host = 'opencode-cli' WHERE host = 'opencode';

ALTER TABLE agent_runs DROP CONSTRAINT IF EXISTS agent_runs_host_check;
ALTER TABLE agent_runs
    ADD CONSTRAINT agent_runs_host_check
    CHECK (host IN ('claude-code', 'codex-cli', 'opencode-cli', 'hermes'));
