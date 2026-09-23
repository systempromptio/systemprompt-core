-- Migration 007 dropped `event_outbox_actor_id_check` on the premise that
-- `event_outbox_actor_id_nonempty` from migration 001 stood beside it. On a
-- database restored from a release snapshot the 001-era ALTER was stamped, not
-- executed, so the auto-named check was the only one and 007 left the column
-- unguarded. Add the named constraint wherever it is missing.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid = 'event_outbox'::regclass
           AND conname = 'event_outbox_actor_id_nonempty'
    ) THEN
        ALTER TABLE event_outbox
            ADD CONSTRAINT event_outbox_actor_id_nonempty CHECK (length(actor_id) > 0);
    END IF;
END $$;
