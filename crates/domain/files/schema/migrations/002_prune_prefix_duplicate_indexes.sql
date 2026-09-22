-- `idx_content_files_content_id` is the leading column of the UNIQUE
-- constraint `content_files_unique (content_id, file_id, role)`, so it
-- answers nothing that constraint's index does not and costs a write on every
-- insert. It is not UNIQUE and backs no constraint of its own.
DROP INDEX IF EXISTS idx_content_files_content_id;
