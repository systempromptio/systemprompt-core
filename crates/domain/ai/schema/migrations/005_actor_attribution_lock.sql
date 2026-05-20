-- Lock the actor attribution invariant on ai_requests.
--
-- See crates/infra/security/schema/migrations/003_actor_attribution_lock.sql
-- for the rationale; the shape mirrors that change so all audit-bearing
-- tables share one vocabulary.

ALTER TABLE ai_requests ALTER COLUMN actor_kind DROP DEFAULT;
