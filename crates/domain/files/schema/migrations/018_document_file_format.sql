-- The `file` format: any upload that is not an editable document (a PDF,
-- an image, an archive) is stored, listed and served as bytes.

ALTER TABLE documents DROP CONSTRAINT IF EXISTS documents_format_check;

ALTER TABLE documents
    ADD CONSTRAINT documents_format_check
    CHECK (format IN ('docx', 'xlsx', 'pptx', 'odt', 'ods', 'odp', 'html', 'markdown', 'text', 'file'));
