-- The actor's display name is stored on each notification when the consumer
-- writes it, so a feed read reads only the notifications table.

ALTER TABLE notifications ADD COLUMN IF NOT EXISTS actor_display TEXT
    CHECK (actor_display IS NULL OR length(actor_display) <= 255);
