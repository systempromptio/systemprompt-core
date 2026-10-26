-- A headless run names the CLI packaging of its host. The OpenCode CLI's id is
-- `opencode-cli`; rows queued under the product id move to it before the
-- column is constrained to the headless host set.
--
-- Why: agent_runs holds one row per headless run and `host` has no index, so
-- the rewrite is one sequential pass over the table. The constraint is added
-- NOT VALID and validated separately, so the scan takes SHARE UPDATE
-- EXCLUSIVE instead of ACCESS EXCLUSIVE.
UPDATE agent_runs SET host = 'opencode-cli' WHERE host = 'opencode';

ALTER TABLE agent_runs DROP CONSTRAINT IF EXISTS agent_runs_host_check;
ALTER TABLE agent_runs
    ADD CONSTRAINT agent_runs_host_check
    CHECK (host IN ('claude-code', 'codex-cli', 'opencode-cli', 'hermes'))
    NOT VALID;
ALTER TABLE agent_runs VALIDATE CONSTRAINT agent_runs_host_check;
