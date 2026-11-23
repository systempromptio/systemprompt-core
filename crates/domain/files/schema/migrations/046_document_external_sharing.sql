-- External sharing: a read-only public link to a document, the password
-- attempts made against one, and an invite addressed to an email outside the
-- instance. A link's and an invite's token is stored only as its SHA-256; a
-- link's password only as its bcrypt hash. The grant audit log gains the
-- event kinds both write. Same DDL as schema/document_public_links.sql,
-- schema/document_link_attempts.sql, schema/document_invites.sql and
-- schema/document_grant_events.sql.
--
-- The three tables are new and empty. The widened event CHECK admits every
-- existing row, so validating it scans and passes.

CREATE TABLE IF NOT EXISTS document_public_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL CHECK (length(token_hash) = 64),
    password_hash TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_by VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    open_count BIGINT NOT NULL DEFAULT 0,
    last_opened_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_public_links_token
    ON document_public_links (token_hash);
CREATE INDEX IF NOT EXISTS idx_document_public_links_document
    ON document_public_links (document_id, created_at);

CREATE TABLE IF NOT EXISTS document_link_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    link_id UUID NOT NULL REFERENCES document_public_links(id) ON DELETE CASCADE,
    client_ip TEXT NOT NULL,
    attempted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_document_link_attempts_window
    ON document_link_attempts (link_id, client_ip, attempted_at);

CREATE TABLE IF NOT EXISTS document_invites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    email TEXT NOT NULL CHECK (email = lower(email) AND length(email) BETWEEN 3 AND 320),
    access_level TEXT NOT NULL CHECK (access_level IN ('view', 'comment')),
    token_hash TEXT NOT NULL CHECK (length(token_hash) = 64),
    invited_by VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    claimed_by_user_id VARCHAR(255),
    claimed_at TIMESTAMPTZ,
    CHECK ((claimed_by_user_id IS NULL) = (claimed_at IS NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_invites_token
    ON document_invites (token_hash);
CREATE INDEX IF NOT EXISTS idx_document_invites_document
    ON document_invites (document_id, created_at);
CREATE INDEX IF NOT EXISTS idx_document_invites_open_email
    ON document_invites (email) WHERE claimed_at IS NULL AND revoked_at IS NULL;

ALTER TABLE document_grant_events DROP CONSTRAINT IF EXISTS document_grant_events_event_check;
ALTER TABLE document_grant_events ADD CONSTRAINT document_grant_events_event_check
    CHECK (event IN ('granted', 'changed', 'revoked', 'revoked_all', 'transferred', 'expired', 'public_link.created', 'public_link.opened', 'public_link.revoked', 'invite.created', 'invite.claimed', 'invite.revoked')) NOT VALID;
ALTER TABLE document_grant_events VALIDATE CONSTRAINT document_grant_events_event_check;
