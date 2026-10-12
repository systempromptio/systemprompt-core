-- Link-click facts (schema/analytics_event_link_facts.sql): the target url
-- and the external flag are stored on the event instead of read out of
-- `event_data` by every traffic query. The columns and index are the base
-- schema's text; every link click that predates them is queued for
-- `refresh_analytics_event_link_facts`, which the
-- `analytics_event_link_facts_rollup` job drains, as is every hour holding a
-- link click for `refresh_analytics_link_click_rollup_hourly`, the hourly
-- page-to-target rollup the traffic navigation report reads. Keys only.
ALTER TABLE analytics_events ADD COLUMN IF NOT EXISTS target_url TEXT;
ALTER TABLE analytics_events ADD COLUMN IF NOT EXISTS is_external BOOLEAN;
CREATE INDEX IF NOT EXISTS idx_analytics_events_link_clicks
    ON analytics_events(timestamp, endpoint, target_url) WHERE event_type = 'link_click';

INSERT INTO projection_backlog (projection, key)
SELECT 'analytics_event_link_facts', id FROM analytics_events
WHERE event_type = 'link_click' AND target_url IS NULL
ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS analytics_link_click_rollup_hourly (
    bucket TIMESTAMPTZ NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    endpoint TEXT NOT NULL,
    target_url TEXT NOT NULL,
    is_external BOOLEAN NOT NULL,
    clicks BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (bucket, user_id, endpoint, target_url, is_external)
);

INSERT INTO projection_backlog (projection, key)
SELECT 'analytics_link_click_rollup_hourly',
       to_char(date_trunc('hour', timestamp, 'UTC') AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24":00:00Z"')
FROM analytics_events
WHERE event_type = 'link_click'
ON CONFLICT DO NOTHING;
