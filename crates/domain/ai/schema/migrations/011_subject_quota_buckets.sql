-- Key quota buckets by an open-vocabulary subject (user, organization,
-- department, ...) instead of user only, and add a cost counter so policies
-- can enforce a spend ceiling. Existing rows keep today's behaviour via the
-- 'user' default.

ALTER TABLE ai_quota_buckets ADD COLUMN IF NOT EXISTS subject_kind TEXT NOT NULL DEFAULT 'user';
ALTER TABLE ai_quota_buckets ADD COLUMN IF NOT EXISTS cost_microdollars BIGINT NOT NULL DEFAULT 0;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'ai_quota_buckets' AND column_name = 'user_id'
    ) THEN
        ALTER TABLE ai_quota_buckets RENAME COLUMN user_id TO subject_id;
    END IF;
END $$;

ALTER TABLE ai_quota_buckets
    DROP CONSTRAINT IF EXISTS ai_quota_buckets_user_id_window_seconds_window_start_key;
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'ai_quota_buckets_subject_key'
          AND table_name = 'ai_quota_buckets'
    ) THEN
        ALTER TABLE ai_quota_buckets
            ADD CONSTRAINT ai_quota_buckets_subject_key
            UNIQUE (subject_kind, subject_id, window_seconds, window_start);
    END IF;
END $$;

DROP INDEX IF EXISTS idx_ai_quota_buckets_user;
CREATE INDEX IF NOT EXISTS idx_ai_quota_buckets_subject
    ON ai_quota_buckets(subject_kind, subject_id);
