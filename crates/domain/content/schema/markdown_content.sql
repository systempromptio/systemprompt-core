CREATE TABLE IF NOT EXISTS markdown_content (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL,
    locale TEXT NOT NULL DEFAULT 'en',

    title TEXT NOT NULL,
    description TEXT NOT NULL,
    body TEXT NOT NULL,

    author TEXT NOT NULL,
    published_at TIMESTAMPTZ NOT NULL,
    keywords TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'article',
    image TEXT,

    category_id TEXT,
    source_id TEXT NOT NULL,

    version_hash TEXT NOT NULL,
    public BOOLEAN NOT NULL DEFAULT true,
    links JSONB NOT NULL DEFAULT '[]'::jsonb,

    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE markdown_content
    ADD COLUMN IF NOT EXISTS locale TEXT NOT NULL DEFAULT 'en';

ALTER TABLE markdown_content DROP CONSTRAINT IF EXISTS markdown_content_slug_key;
DROP INDEX IF EXISTS idx_markdown_content_slug;

CREATE INDEX IF NOT EXISTS idx_markdown_content_category ON markdown_content(category_id);
CREATE INDEX IF NOT EXISTS idx_markdown_content_source ON markdown_content(source_id);
CREATE INDEX IF NOT EXISTS idx_markdown_content_published ON markdown_content(published_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_markdown_content_slug_locale ON markdown_content(slug, locale);
CREATE INDEX IF NOT EXISTS idx_markdown_content_locale ON markdown_content(locale);
CREATE INDEX IF NOT EXISTS idx_markdown_content_version_hash ON markdown_content(version_hash);
CREATE INDEX IF NOT EXISTS idx_markdown_content_kind ON markdown_content(kind);
CREATE INDEX IF NOT EXISTS idx_markdown_content_links ON markdown_content USING GIN (links);
CREATE INDEX IF NOT EXISTS idx_markdown_content_public ON markdown_content(public) WHERE public = true;
