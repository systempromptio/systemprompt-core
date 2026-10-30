-- A notification carries the actor's id, a title and a body, and nothing
-- derived from them: clients resolve the actor's name from the users API and
-- truncate for display. Each added column carries a constant default; the
-- existing excerpt text becomes the body before the column is dropped.

ALTER TABLE notifications ADD COLUMN IF NOT EXISTS title TEXT NOT NULL DEFAULT '';
ALTER TABLE notifications ADD COLUMN IF NOT EXISTS body TEXT NOT NULL DEFAULT '';
-- Why: bounded: the excerpt column exists only on 0.76 databases, which hold at most one release of notification rows.
UPDATE notifications SET body = excerpt WHERE excerpt IS NOT NULL AND excerpt <> '' AND body = '';
ALTER TABLE notifications DROP COLUMN IF EXISTS excerpt;
ALTER TABLE notifications DROP COLUMN IF EXISTS actor_display;
