-- ai_requests.session_id historically had no foreign key, so deleted sessions
-- left orphaned request rows (3,618 observed in production on 2026-07-23).
-- Null the dangling references, then enforce integrity going forward.
UPDATE ai_requests ar
SET session_id = NULL
WHERE ar.session_id IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM user_sessions us WHERE us.session_id = ar.session_id
  );

ALTER TABLE ai_requests
    DROP CONSTRAINT IF EXISTS ai_requests_session_id_fkey;
ALTER TABLE ai_requests
    ADD CONSTRAINT ai_requests_session_id_fkey
    FOREIGN KEY (session_id) REFERENCES user_sessions(session_id) ON DELETE SET NULL;
