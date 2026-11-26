-- The `analytics` report views over the `content` member's tables: the read
-- contract the analytics reports query, contributed by the member that owns
-- the source tables, so a composition without that member has neither the
-- tables nor their views. They follow the rules of the analytics member's
-- report_views.sql: each names its columns, rows of a user whose status is
-- `deleted` are invisible, and each is replaced in place (a change that
-- drops, renames or reorders a column ships a migration that drops the view
-- first).

CREATE OR REPLACE VIEW report_markdown_content AS
SELECT
    src.id, src.slug, src.locale, src.title, src.description, src.body,
    src.author, src.published_at, src.keywords, src.kind, src.image,
    src.category_id, src.source_id, src.version_hash, src.public, src.links,
    src.updated_at
FROM markdown_content src;
