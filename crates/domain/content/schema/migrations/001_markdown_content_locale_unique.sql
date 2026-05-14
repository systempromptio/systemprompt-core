-- Backfill locale column and replace slug-only uniqueness with (slug, locale).
ALTER TABLE markdown_content
    ADD COLUMN IF NOT EXISTS locale TEXT NOT NULL DEFAULT 'en';

ALTER TABLE markdown_content DROP CONSTRAINT IF EXISTS markdown_content_slug_key;
DROP INDEX IF EXISTS idx_markdown_content_slug;
