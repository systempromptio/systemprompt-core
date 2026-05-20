-- Actor attribution for the cross-replica event outbox.
--
-- See crates/infra/security/schema/migrations/002_actor_attribution.sql for
-- the rationale; the columns and constraints mirror that shape so all
-- audit-bearing tables share one vocabulary.

ALTER TABLE event_outbox
    ADD COLUMN IF NOT EXISTS actor_kind TEXT NOT NULL DEFAULT 'user',
    ADD COLUMN IF NOT EXISTS actor_id TEXT;

UPDATE event_outbox
   SET actor_id = user_id
 WHERE actor_id IS NULL;

ALTER TABLE event_outbox
    ALTER COLUMN actor_id SET NOT NULL;

ALTER TABLE event_outbox
    DROP CONSTRAINT IF EXISTS event_outbox_actor_kind_check;
ALTER TABLE event_outbox
    ADD CONSTRAINT event_outbox_actor_kind_check
    CHECK (actor_kind IN ('user', 'job', 'mcp'));

ALTER TABLE event_outbox
    DROP CONSTRAINT IF EXISTS event_outbox_actor_id_nonempty;
ALTER TABLE event_outbox
    ADD CONSTRAINT event_outbox_actor_id_nonempty
    CHECK (length(actor_id) > 0);

CREATE INDEX IF NOT EXISTS idx_event_outbox_actor
    ON event_outbox(actor_kind, actor_id);
