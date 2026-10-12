-- Link clicks per UTC hour, user, page and target (see
-- analytics_event_link_facts.sql for the triggers and projector that keep
-- it, and the paradigm note there).
CREATE TABLE IF NOT EXISTS analytics_link_click_rollup_hourly (
    bucket TIMESTAMPTZ NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    endpoint TEXT NOT NULL,
    target_url TEXT NOT NULL,
    is_external BOOLEAN NOT NULL,
    clicks BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (bucket, user_id, endpoint, target_url, is_external)
);
