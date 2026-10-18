-- Lock the actor attribution invariant on governance_decisions.
--
-- Migration 002 left a DEFAULT 'user' on actor_kind so the backfill
-- could land before every writer was updated. All writers now pass an
-- explicit Actor, so the DEFAULT is dropped: any future insert that
-- omits actor metadata fails at the DB. actor_id never had a DEFAULT.

ALTER TABLE governance_decisions ALTER COLUMN actor_kind DROP DEFAULT;
