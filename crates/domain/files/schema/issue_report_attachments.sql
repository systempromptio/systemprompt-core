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
