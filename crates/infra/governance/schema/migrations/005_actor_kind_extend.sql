-- Extend `governance_decisions.actor_kind` CHECK to cover every
-- `systemprompt_identifiers::ActorKind` variant: `Anonymous`, `System`,
-- and `Agent` join the previously-allowed `User`, `Job`, `Mcp`.
--
-- Idempotent; safe to re-run.

ALTER TABLE governance_decisions
    DROP CONSTRAINT IF EXISTS governance_decisions_actor_kind_check;
ALTER TABLE governance_decisions
    ADD CONSTRAINT governance_decisions_actor_kind_check
    CHECK (actor_kind IN ('user', 'anonymous', 'system', 'job', 'mcp', 'agent'));
