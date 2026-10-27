-- Document store tables, created on a database installed before they existed.
-- The base schema files declare the same tables.

CREATE TABLE IF NOT EXISTS document_folders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    parent_id UUID REFERENCES document_folders(id) ON DELETE RESTRICT,
    name TEXT NOT NULL CHECK (length(name) > 0),
    ancestor_ids UUID[] NOT NULL DEFAULT '{}',
    owner_user_id VARCHAR(255) NOT NULL,
    created_by VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_folders_parent_name
    ON document_folders (parent_id, name) NULLS NOT DISTINCT
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_document_folders_ancestor_ids
    ON document_folders USING gin (ancestor_ids);
CREATE INDEX IF NOT EXISTS idx_document_folders_owner
    ON document_folders (owner_user_id) WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS document_blobs (
    content_sha256 TEXT PRIMARY KEY CHECK (content_sha256 ~ '^[0-9a-f]{64}$'),
    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),
    storage_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_document_blobs_created_at ON document_blobs (created_at);

CREATE TABLE IF NOT EXISTS documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    folder_id UUID REFERENCES document_folders(id) ON DELETE RESTRICT,
    name TEXT NOT NULL CHECK (length(name) > 0),
    format TEXT NOT NULL CHECK (format IN ('docx', 'xlsx', 'pptx', 'odt', 'ods', 'odp', 'html', 'markdown', 'text')),
    mime_type TEXT NOT NULL,
    head_revision_id UUID,
    template_id TEXT,
    owner_user_id VARCHAR(255) NOT NULL,
    created_by VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_documents_folder_name
    ON documents (folder_id, name) NULLS NOT DISTINCT
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_documents_updated
    ON documents (updated_at DESC, id DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_documents_owner_updated
    ON documents (owner_user_id, updated_at DESC, id DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_documents_folder_id
    ON documents (folder_id) WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS document_revisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    parent_revision_id UUID REFERENCES document_revisions(id) ON DELETE SET NULL,
    content_sha256 TEXT NOT NULL REFERENCES document_blobs(content_sha256) ON DELETE RESTRICT,
    author_user_id VARCHAR(255) NOT NULL,
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent', 'bridge', 'import', 'system')),
    origin_kind TEXT NOT NULL CHECK (origin_kind IN ('mcp_execution', 'bridge_device', 'web_session', 'edit_session', 'api')),
    origin_id TEXT,
    message TEXT,
    edit_ops JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Why: the table is created empty by this migration, so the index builds instantly.
CREATE INDEX IF NOT EXISTS idx_document_revisions_document_created
    ON document_revisions (document_id, created_at DESC, id DESC);
-- Why: the table is created empty by this migration, so the index builds instantly.
CREATE INDEX IF NOT EXISTS idx_document_revisions_created
    ON document_revisions (created_at, id);
-- Why: the table is created empty by this migration, so the index builds instantly.
CREATE INDEX IF NOT EXISTS idx_document_revisions_content_sha256
    ON document_revisions (content_sha256);

CREATE TABLE IF NOT EXISTS document_grants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID REFERENCES documents(id) ON DELETE CASCADE,
    folder_id UUID REFERENCES document_folders(id) ON DELETE CASCADE,
    subject_kind TEXT NOT NULL CHECK (length(subject_kind) > 0),
    subject_value TEXT NOT NULL CHECK (length(subject_value) > 0),
    access_level TEXT NOT NULL CHECK (access_level IN ('view', 'comment', 'edit', 'share', 'own')),
    granted_by VARCHAR(255) NOT NULL,
    valid_until TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT document_grants_one_target CHECK ((document_id IS NULL) <> (folder_id IS NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_grants_target_subject
    ON document_grants (document_id, folder_id, subject_kind, subject_value) NULLS NOT DISTINCT;
CREATE INDEX IF NOT EXISTS idx_document_grants_subject
    ON document_grants (subject_kind, subject_value);
CREATE INDEX IF NOT EXISTS idx_document_grants_folder_id
    ON document_grants (folder_id) WHERE folder_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS document_locks (
    document_id UUID PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    holder_user_id VARCHAR(255) NOT NULL,
    holder_kind TEXT NOT NULL CHECK (holder_kind IN ('bridge', 'editor', 'agent')),
    holder_id TEXT NOT NULL CHECK (length(holder_id) > 0),
    acquired_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_document_locks_expires_at ON document_locks (expires_at);

CREATE TABLE IF NOT EXISTS document_presence (
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    user_id VARCHAR(255) NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('bridge', 'agent', 'web')),
    state TEXT NOT NULL CHECK (state IN ('viewing', 'editing')),
    cursor JSONB,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (document_id, user_id, source)
);

-- Why: the table is created empty by this migration, so the index builds instantly.
CREATE INDEX IF NOT EXISTS idx_document_presence_expires_at ON document_presence (expires_at);

CREATE TABLE IF NOT EXISTS document_edit_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    base_revision_id UUID REFERENCES document_revisions(id) ON DELETE SET NULL,
    snapshot_sha256 TEXT REFERENCES document_blobs(content_sha256) ON DELETE RESTRICT,
    checkpoint_revision_id UUID REFERENCES document_revisions(id) ON DELETE SET NULL,
    opened_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    closed_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_edit_sessions_open
    ON document_edit_sessions (document_id) WHERE closed_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_document_edit_sessions_snapshot
    ON document_edit_sessions (snapshot_sha256) WHERE snapshot_sha256 IS NOT NULL;

CREATE TABLE IF NOT EXISTS document_text_index (
    document_id UUID PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    revision_id UUID NOT NULL REFERENCES document_revisions(id) ON DELETE CASCADE,
    plain_text TEXT NOT NULL,
    body TSVECTOR NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_document_text_index_body ON document_text_index USING gin (body);

