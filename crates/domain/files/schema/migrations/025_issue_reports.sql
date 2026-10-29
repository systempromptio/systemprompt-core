-- Issue reports: a user's or agent's bug and feedback reports, folded by
-- fingerprint while open, and the upload blobs they carry, which the blob
-- garbage collector keeps while a report references them.

CREATE TABLE IF NOT EXISTS issue_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    reporter_user_id VARCHAR(255) NOT NULL,
    via_agent TEXT CHECK (via_agent IS NULL OR length(via_agent) BETWEEN 1 AND 64),
    kind TEXT NOT NULL CHECK (kind IN ('bug', 'feedback')),
    severity TEXT NOT NULL CHECK (severity IN ('low', 'medium', 'high', 'critical')),
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'triaged', 'needs_info', 'resolved', 'closed')),
    title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
    body TEXT NOT NULL DEFAULT '' CHECK (length(body) <= 20000),
    fingerprint TEXT NOT NULL CHECK (fingerprint ~ '^[0-9a-f]{64}$'),
    occurrences INTEGER NOT NULL DEFAULT 1 CHECK (occurrences >= 1),
    trace_id TEXT,
    context_id TEXT,
    session_id TEXT,
    task_id TEXT,
    mcp_execution_id TEXT,
    client JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (pg_column_size(client) <= 8192),
    triage_note TEXT CHECK (triage_note IS NULL OR length(triage_note) <= 4000),
    triaged_by VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    closed_at TIMESTAMPTZ,
    CHECK ((status IN ('resolved', 'closed')) = (closed_at IS NOT NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_issue_reports_open_fingerprint
    ON issue_reports (reporter_user_id, fingerprint)
    WHERE status IN ('open', 'triaged', 'needs_info');
CREATE INDEX IF NOT EXISTS idx_issue_reports_reporter
    ON issue_reports (reporter_user_id, last_seen_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_issue_reports_status
    ON issue_reports (status, last_seen_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_issue_reports_closed
    ON issue_reports (closed_at) WHERE closed_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS issue_report_attachments (
    report_id UUID NOT NULL REFERENCES issue_reports(id) ON DELETE CASCADE,
    upload_id UUID NOT NULL,
    content_sha256 TEXT NOT NULL REFERENCES document_blobs(content_sha256) ON DELETE RESTRICT,
    file_name TEXT NOT NULL CHECK (length(file_name) BETWEEN 1 AND 255),
    mime_type TEXT NOT NULL,
    size_bytes BIGINT NOT NULL CHECK (size_bytes > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (report_id, upload_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_issue_report_attachments_upload
    ON issue_report_attachments (upload_id);
CREATE INDEX IF NOT EXISTS idx_issue_report_attachments_blob
    ON issue_report_attachments (content_sha256);
