-- Drop every index on this crate's tables whose column list is a strict
-- prefix of another, non-partial index on the same table. Three of these
-- duplicate a UNIQUE constraint outright.
--
-- None is UNIQUE and none backs a constraint.

-- covered by campaign_links_short_code_key (same column)
DROP INDEX IF EXISTS idx_campaign_links_short_code;
-- covered by content_performance_metrics_content_id_key (same column)
DROP INDEX IF EXISTS idx_content_performance_metrics_content_id;
-- covered by idx_link_clicks_link_session (link_id, session_id)
DROP INDEX IF EXISTS idx_link_clicks_link_id;
-- covered by markdown_categories_slug_key (same column)
DROP INDEX IF EXISTS idx_markdown_categories_slug;
