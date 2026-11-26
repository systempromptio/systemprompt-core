-- Content audience rollups (schema/content_audience_rollups.sql). The tables
-- are the base schema's text; the hours that predate them are queued, keys
-- only, for `refresh_content_engagement_rollup_hourly` and
-- `refresh_analytics_page_view_rollup_hourly`, which the `backfill_rollups`
-- job drains. Rows written after this migration are rolled up by the
-- statement triggers, and a queued hour is rebuilt whole from the ledger.
CREATE TABLE IF NOT EXISTS content_engagement_rollup_hourly (
    bucket TIMESTAMPTZ NOT NULL,
    content_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    views BIGINT NOT NULL DEFAULT 0,
    time_on_page_sum_ms BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (bucket, content_id, session_id, user_id)
);
CREATE INDEX IF NOT EXISTS idx_content_engagement_rollup_hourly_session
    ON content_engagement_rollup_hourly(session_id);

CREATE TABLE IF NOT EXISTS analytics_page_view_rollup_hourly (
    bucket TIMESTAMPTZ NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    endpoint TEXT NOT NULL,
    event_category TEXT NOT NULL,
    views BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (bucket, user_id, endpoint, event_category)
);
CREATE INDEX IF NOT EXISTS idx_analytics_page_view_rollup_hourly_category_bucket
    ON analytics_page_view_rollup_hourly(event_category, bucket);

INSERT INTO projection_backlog (projection, key)
SELECT 'content_engagement_rollup_hourly',
       to_char(date_trunc('hour', created_at, 'UTC') AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24":00:00Z"')
FROM engagement_events
WHERE content_id IS NOT NULL
ON CONFLICT DO NOTHING;

INSERT INTO projection_backlog (projection, key)
SELECT 'analytics_page_view_rollup_hourly',
       to_char(date_trunc('hour', timestamp, 'UTC') AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24":00:00Z"')
FROM analytics_events
WHERE event_type = 'page_view'
ON CONFLICT DO NOTHING;
