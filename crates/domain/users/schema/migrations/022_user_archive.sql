-- Archive instead of delete (NFR-5.2). Deleting a user now sets status
-- 'deleted' plus who archived them, when and why, and revokes their
-- sessions, API keys and device certificates; the row and every record that
-- keys on it stay. `database_cleanup` purges an archive physically once it is
-- older than `retention.archived_users_days`, never while `legal_hold` is set.
-- Rows already 'deleted' before this migration keep a NULL archived_at: they
-- are restorable but never purged automatically, because no one recorded when
-- they were removed. Same DDL as schema/users.sql.
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS archived_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS archived_by TEXT,
    ADD COLUMN IF NOT EXISTS archive_reason TEXT,
    ADD COLUMN IF NOT EXISTS legal_hold BOOLEAN NOT NULL DEFAULT false;
CREATE INDEX IF NOT EXISTS idx_users_archived_at ON users(archived_at) WHERE archived_at IS NOT NULL;
