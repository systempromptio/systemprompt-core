-- Actor attribution for governance decisions.
--
-- Every governance decision row now records both the accountable user
-- (`user_id`, unchanged) and the surface that produced the action
-- (`actor_kind` + `actor_id`). Direct human actions write
-- ('user', user_id); jobs write ('job', <job_name>); MCP system-invoked
-- tool calls write ('mcp', <server_name>).
--
-- This migration is additive: existing rows are backfilled to
-- ('user', user_id) — the user IS the actor when no intermediate surface
-- was involved. A follow-up migration drops the DEFAULT clauses once all
-- writers explicitly specify the actor.

ALTER TABLE governance_decisions
    ADD COLUMN IF NOT EXISTS actor_kind TEXT NOT NULL DEFAULT 'user',
    ADD COLUMN IF NOT EXISTS actor_id TEXT;

UPDATE governance_decisions
   SET actor_id = user_id
 WHERE actor_id IS NULL;

ALTER TABLE governance_decisions
    ALTER COLUMN actor_id SET NOT NULL;

ALTER TABLE governance_decisions
    DROP CONSTRAINT IF EXISTS governance_decisions_actor_kind_check;
ALTER TABLE governance_decisions
    ADD CONSTRAINT governance_decisions_actor_kind_check
    CHECK (actor_kind IN ('user', 'job', 'mcp'));

ALTER TABLE governance_decisions
    DROP CONSTRAINT IF EXISTS governance_decisions_actor_id_nonempty;
ALTER TABLE governance_decisions
    ADD CONSTRAINT governance_decisions_actor_id_nonempty
    CHECK (length(actor_id) > 0);

CREATE INDEX IF NOT EXISTS idx_governance_decisions_actor
    ON governance_decisions(actor_kind, actor_id);
