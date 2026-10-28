-- Bytes a session has accepted across part PUTs, re-sends included, so a
-- session's part budget (twice its declared size) bounds the I/O one
-- upload can cost.

ALTER TABLE document_uploads
    ADD COLUMN IF NOT EXISTS received_bytes BIGINT NOT NULL DEFAULT 0;
