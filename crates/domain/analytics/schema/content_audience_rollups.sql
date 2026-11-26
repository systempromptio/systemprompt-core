-- Content audience rollups, written at write time (store, don't compute), so
-- the content audience reports read the hours in their window instead of
-- grouping the engagement and event ledgers on every load.
--
--   content_engagement_rollup_hourly   engagement events per UTC hour,
--                                      content and session: views and the
--                                      time on page capped at 30 minutes
--   analytics_page_view_rollup_hourly  page-view events per UTC hour, user,
--                                      page and event category
--
-- Sessions and users are dimensions so the report views apply the traffic
-- and person filters at read time: a session reclassified as a bot, or a user
-- flagged as one, leaves the reports without rewriting a rollup row. An
-- absent page or category is stored as ''.
--
-- Statement triggers keep both current: an insert adds its rows, a delete
-- subtracts them, an update subtracts the old version and adds the new one
-- for rows whose rolled-up columns changed; a row that falls to zero is
-- removed. `refresh_content_engagement_rollup_hourly(keys)` and
-- `refresh_analytics_page_view_rollup_hourly(keys)` rebuild whole hours (keys
-- are `YYYY-MM-DDTHH:00:00Z`) from the ledger; migration 019 queues every hour
-- that predates the tables, and the `backfill_rollups` job drains them.

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

CREATE OR REPLACE FUNCTION content_engagement_rollup_upsert_sql(source TEXT)
RETURNS TEXT LANGUAGE sql IMMUTABLE AS $$
SELECT format('
    INSERT INTO content_engagement_rollup_hourly AS r (
        bucket, content_id, session_id, user_id, views, time_on_page_sum_ms)
    SELECT date_trunc(''hour'', s.created_at, ''UTC''), s.content_id, s.session_id, s.user_id,
           SUM(s.sign), SUM(s.sign * LEAST(s.time_on_page_ms, 1800000))
    FROM (%s) s
    WHERE s.content_id IS NOT NULL
    GROUP BY 1, 2, 3, 4
    ORDER BY 1, 2, 3, 4
    ON CONFLICT (bucket, content_id, session_id, user_id) DO UPDATE SET
        views = r.views + EXCLUDED.views,
        time_on_page_sum_ms = r.time_on_page_sum_ms + EXCLUDED.time_on_page_sum_ms
', source)
$$;

CREATE OR REPLACE FUNCTION refresh_content_engagement_rollup_hourly(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE hours TIMESTAMPTZ[]; written BIGINT;
BEGIN
    hours := ARRAY(SELECT DISTINCT date_trunc('hour', k::timestamptz, 'UTC') FROM unnest(keys) k);
    DELETE FROM content_engagement_rollup_hourly WHERE bucket = ANY(hours);
    EXECUTE content_engagement_rollup_upsert_sql(
        'SELECT e.*, 1 AS sign FROM engagement_events e JOIN unnest($1) h(b) '
        'ON e.created_at >= h.b AND e.created_at < h.b + interval ''1 hour''')
    USING hours;
    GET DIAGNOSTICS written = ROW_COUNT;
    RETURN written;
END $$;

CREATE OR REPLACE FUNCTION content_engagement_rollup_apply()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    changed CONSTANT TEXT := '(n.created_at, n.content_id, n.session_id, n.user_id, n.time_on_page_ms) '
        'IS DISTINCT FROM (o.created_at, o.content_id, o.session_id, o.user_id, o.time_on_page_ms)';
BEGIN
    IF TG_OP = 'INSERT' THEN
        EXECUTE content_engagement_rollup_upsert_sql('SELECT n.*, 1 AS sign FROM new_rows n');
        RETURN NULL;
    ELSIF TG_OP = 'UPDATE' THEN
        EXECUTE content_engagement_rollup_upsert_sql(
            'SELECT n.*, 1 AS sign FROM new_rows n JOIN old_rows o ON o.id = n.id WHERE ' || changed
            || ' UNION ALL SELECT o.*, -1 AS sign FROM old_rows o JOIN new_rows n ON n.id = o.id WHERE '
            || changed);
    ELSE
        EXECUTE content_engagement_rollup_upsert_sql('SELECT o.*, -1 AS sign FROM old_rows o');
    END IF;
    DELETE FROM content_engagement_rollup_hourly
    WHERE views = 0
      AND bucket = ANY(ARRAY(SELECT DISTINCT date_trunc('hour', o.created_at, 'UTC') FROM old_rows o));
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS content_engagement_rollup_insert ON engagement_events;
CREATE TRIGGER content_engagement_rollup_insert
    AFTER INSERT ON engagement_events
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION content_engagement_rollup_apply();

DROP TRIGGER IF EXISTS content_engagement_rollup_update ON engagement_events;
CREATE TRIGGER content_engagement_rollup_update
    AFTER UPDATE ON engagement_events
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION content_engagement_rollup_apply();

DROP TRIGGER IF EXISTS content_engagement_rollup_delete ON engagement_events;
CREATE TRIGGER content_engagement_rollup_delete
    AFTER DELETE ON engagement_events
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION content_engagement_rollup_apply();

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

CREATE OR REPLACE FUNCTION analytics_page_view_rollup_upsert_sql(source TEXT)
RETURNS TEXT LANGUAGE sql IMMUTABLE AS $$
SELECT format('
    INSERT INTO analytics_page_view_rollup_hourly AS r (bucket, user_id, endpoint, event_category, views)
    SELECT date_trunc(''hour'', s.timestamp, ''UTC''), s.user_id, COALESCE(s.endpoint, ''''),
           COALESCE(s.event_category, ''''), SUM(s.sign)
    FROM (%s) s
    WHERE s.event_type = ''page_view''
    GROUP BY 1, 2, 3, 4
    ORDER BY 1, 2, 3, 4
    ON CONFLICT (bucket, user_id, endpoint, event_category) DO UPDATE
    SET views = r.views + EXCLUDED.views
', source)
$$;

CREATE OR REPLACE FUNCTION refresh_analytics_page_view_rollup_hourly(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE hours TIMESTAMPTZ[]; written BIGINT;
BEGIN
    hours := ARRAY(SELECT DISTINCT date_trunc('hour', k::timestamptz, 'UTC') FROM unnest(keys) k);
    DELETE FROM analytics_page_view_rollup_hourly WHERE bucket = ANY(hours);
    EXECUTE analytics_page_view_rollup_upsert_sql(
        'SELECT e.*, 1 AS sign FROM analytics_events e JOIN unnest($1) h(b) '
        'ON e.timestamp >= h.b AND e.timestamp < h.b + interval ''1 hour''')
    USING hours;
    GET DIAGNOSTICS written = ROW_COUNT;
    RETURN written;
END $$;

CREATE OR REPLACE FUNCTION analytics_page_view_rollup_apply()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    changed CONSTANT TEXT := '(n.timestamp, n.user_id, n.endpoint, n.event_category, n.event_type) '
        'IS DISTINCT FROM (o.timestamp, o.user_id, o.endpoint, o.event_category, o.event_type)';
BEGIN
    IF TG_OP = 'INSERT' THEN
        EXECUTE analytics_page_view_rollup_upsert_sql('SELECT n.*, 1 AS sign FROM new_rows n');
        RETURN NULL;
    ELSIF TG_OP = 'UPDATE' THEN
        EXECUTE analytics_page_view_rollup_upsert_sql(
            'SELECT n.*, 1 AS sign FROM new_rows n JOIN old_rows o ON o.id = n.id WHERE ' || changed
            || ' UNION ALL SELECT o.*, -1 AS sign FROM old_rows o JOIN new_rows n ON n.id = o.id WHERE '
            || changed);
    ELSE
        EXECUTE analytics_page_view_rollup_upsert_sql('SELECT o.*, -1 AS sign FROM old_rows o');
    END IF;
    DELETE FROM analytics_page_view_rollup_hourly
    WHERE views = 0
      AND bucket = ANY(ARRAY(SELECT DISTINCT date_trunc('hour', o.timestamp, 'UTC') FROM old_rows o));
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS analytics_page_view_rollup_insert ON analytics_events;
CREATE TRIGGER analytics_page_view_rollup_insert
    AFTER INSERT ON analytics_events
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION analytics_page_view_rollup_apply();

DROP TRIGGER IF EXISTS analytics_page_view_rollup_update ON analytics_events;
CREATE TRIGGER analytics_page_view_rollup_update
    AFTER UPDATE ON analytics_events
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION analytics_page_view_rollup_apply();

DROP TRIGGER IF EXISTS analytics_page_view_rollup_delete ON analytics_events;
CREATE TRIGGER analytics_page_view_rollup_delete
    AFTER DELETE ON analytics_events
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION analytics_page_view_rollup_apply();
