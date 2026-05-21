-- Extend `governance_decisions.actor_kind` CHECK to cover every
-- `ActorKind` variant. The `Anonymous`, `System`, and `Agent` variants
-- were added to `systemprompt_identifiers::ActorKind` after the original
-- attribution migration (002) shipped, but the constraint was not
-- updated alongside the enum. Hook-endpoint writes carrying `agent_id`
-- resolve to `Actor::agent(...)` (actor_kind = 'agent') and were
-- silently rejected by the old CHECK, dropping every governance
-- decision routed through `POST /api/public/hooks/govern`.
--
-- This migration re-aligns the schema with the enum. It is idempotent
-- and safe to re-run.

ALTER TABLE governance_decisions
    DROP CONSTRAINT IF EXISTS governance_decisions_actor_kind_check;
ALTER TABLE governance_decisions
    ADD CONSTRAINT governance_decisions_actor_kind_check
    CHECK (actor_kind IN ('user', 'anonymous', 'system', 'job', 'mcp', 'agent'));
