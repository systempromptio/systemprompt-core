-- Mentions share: a mention of someone without access grants them comment
-- access in the comment's own statement, the grant's audit row says why and
-- which agent acted, and the comment records the agent that wrote it.

ALTER TABLE document_grant_events
    ADD COLUMN IF NOT EXISTS reason TEXT;

ALTER TABLE document_grant_events
    ADD COLUMN IF NOT EXISTS via_agent TEXT
        CHECK (via_agent IS NULL OR length(via_agent) BETWEEN 1 AND 64);

ALTER TABLE document_comments
    ADD COLUMN IF NOT EXISTS via_agent TEXT
        CHECK (via_agent IS NULL OR length(via_agent) BETWEEN 1 AND 64);
