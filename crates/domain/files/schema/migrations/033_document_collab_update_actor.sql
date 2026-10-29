-- Live co-editing: each logged update records the actor it ran as and the
-- access level it was admitted at, and each session records the sequence
-- number its last checkpoint covers, so any replica can fold the checkpoint
-- actor and pick the checkpoint author from the shared log.

ALTER TABLE document_collab_updates
    ADD COLUMN IF NOT EXISTS actor_kind TEXT,
    ADD COLUMN IF NOT EXISTS actor_name TEXT,
    ADD COLUMN IF NOT EXISTS actor_attended BOOLEAN,
    ADD COLUMN IF NOT EXISTS edit_level TEXT;

ALTER TABLE document_collab_updates
    DROP CONSTRAINT IF EXISTS document_collab_updates_actor_kind_check;
ALTER TABLE document_collab_updates
    ADD CONSTRAINT document_collab_updates_actor_kind_check
    CHECK (actor_kind IS NULL OR actor_kind IN ('human', 'agent', 'system')) NOT VALID;
ALTER TABLE document_collab_updates
    VALIDATE CONSTRAINT document_collab_updates_actor_kind_check;

ALTER TABLE document_collab_updates
    DROP CONSTRAINT IF EXISTS document_collab_updates_edit_level_check;
ALTER TABLE document_collab_updates
    ADD CONSTRAINT document_collab_updates_edit_level_check
    CHECK (edit_level IS NULL OR edit_level IN ('edit', 'comment')) NOT VALID;
ALTER TABLE document_collab_updates
    VALIDATE CONSTRAINT document_collab_updates_edit_level_check;

ALTER TABLE document_edit_sessions
    ADD COLUMN IF NOT EXISTS checkpoint_seq BIGINT NOT NULL DEFAULT 0;
