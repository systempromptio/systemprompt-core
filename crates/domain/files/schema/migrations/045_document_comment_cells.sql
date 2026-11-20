-- document_comments.anchor_cells: the A1 cell range (`B2` or `B2:D9`) a
-- comment on a spreadsheet's sheet node points at. A range is exclusive
-- with a quote, so a row never carries both anchor_cells and anchor_find;
-- the service also refuses it beside anchor_nth or a cursor range. A
-- detached comment loses its range as it loses its quote.
--
-- No backfill: a migration past the files floor is DDL only. The column is
-- new and NULL on every row, so validating the constraint scans and passes.

ALTER TABLE document_comments
    ADD COLUMN IF NOT EXISTS anchor_cells TEXT;

ALTER TABLE document_comments DROP CONSTRAINT IF EXISTS document_comments_anchor_cells;
ALTER TABLE document_comments ADD CONSTRAINT document_comments_anchor_cells
    CHECK (anchor_cells IS NULL OR anchor_find IS NULL) NOT VALID;
ALTER TABLE document_comments VALIDATE CONSTRAINT document_comments_anchor_cells;
