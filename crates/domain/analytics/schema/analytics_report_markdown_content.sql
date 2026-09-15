CREATE TABLE IF NOT EXISTS analytics_report_markdown_content (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL,
    title TEXT NOT NULL,
    source_id TEXT NOT NULL
);
