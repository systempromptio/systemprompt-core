-- Actor attribution for ai_requests.
--
-- See crates/infra/security/schema/migrations/002_actor_attribution.sql for
-- the rationale; the columns and constraints mirror that shape so all
-- audit-bearing tables share one vocabulary.

ALTER TABLE ai_requests
    ADD COLUMN IF NOT EXISTS actor_kind TEXT NOT NULL DEFAULT 'user',
    ADD COLUMN IF NOT EXISTS actor_id TEXT;

UPDATE ai_requests
   SET actor_id = user_id
 WHERE actor_id IS NULL;

ALTER TABLE ai_requests
    ALTER COLUMN actor_id SET NOT NULL;

ALTER TABLE ai_requests
    DROP CONSTRAINT IF EXISTS ai_requests_actor_kind_check;
ALTER TABLE ai_requests
    ADD CONSTRAINT ai_requests_actor_kind_check
    CHECK (actor_kind IN ('user', 'job', 'mcp'));

ALTER TABLE ai_requests
    DROP CONSTRAINT IF EXISTS ai_requests_actor_id_nonempty;
ALTER TABLE ai_requests
    ADD CONSTRAINT ai_requests_actor_id_nonempty
    CHECK (length(actor_id) > 0);

CREATE INDEX IF NOT EXISTS idx_ai_requests_actor
    ON ai_requests(actor_kind, actor_id);
