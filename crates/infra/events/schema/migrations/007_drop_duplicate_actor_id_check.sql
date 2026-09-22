-- `event_outbox.actor_id` carried the same CHECK twice on established
-- databases: the inline column check (auto-named `event_outbox_actor_id_check`)
-- and `event_outbox_actor_id_nonempty` from migration 001. The base schema now
-- declares the named one inline; drop the auto-named twin so every install
-- ends with the same single constraint.
ALTER TABLE event_outbox DROP CONSTRAINT IF EXISTS event_outbox_actor_id_check;
