-- Link-click facts: the target of a `link_click` event and whether it left
-- the site, stored on the row instead of pulled out of `event_data` by every
-- traffic query (store, don't compute).
--
--   analytics_events.target_url   `event_data->>'target_url'`
--   analytics_events.is_external  `event_data->>'is_external' = 'true'`
--
-- NULL on an event that is not a link click, and on a link click recorded
-- before the columns existed until the `analytics_event_link_facts_rollup`
-- job stamps it. The statement trigger stamps the rows each insert wrote;
-- `refresh_analytics_event_link_facts(keys)` applies the same rule to queued
-- event ids; migration 011 queues the link clicks that predate the columns.
CREATE OR REPLACE FUNCTION refresh_analytics_event_link_facts(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE written BIGINT;
BEGIN
    UPDATE analytics_events e
    SET target_url = e.event_data->>'target_url',
        is_external = COALESCE(e.event_data->>'is_external', 'false') = 'true'
    WHERE e.id = ANY(keys) AND e.event_type = 'link_click'
      AND (e.target_url, e.is_external) IS DISTINCT FROM
          (e.event_data->>'target_url', COALESCE(e.event_data->>'is_external', 'false') = 'true');
    GET DIAGNOSTICS written = ROW_COUNT;
    RETURN written;
END $$;

CREATE OR REPLACE FUNCTION stamp_analytics_event_link_facts()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE keys TEXT[];
BEGIN
    keys := ARRAY(SELECT n.id FROM new_rows n WHERE n.event_type = 'link_click' AND n.target_url IS NULL);
    IF cardinality(keys) > 0 THEN PERFORM refresh_analytics_event_link_facts(keys); END IF;
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS analytics_event_link_facts_insert ON analytics_events;
CREATE TRIGGER analytics_event_link_facts_insert
    AFTER INSERT ON analytics_events
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION stamp_analytics_event_link_facts();

-- Link clicks rolled up per UTC hour, from page to target, so the traffic
-- navigation report reads the hours in its window instead of grouping the
-- event ledger. The insert trigger adds each statement's link clicks in the
-- writer's transaction and a delete subtracts them; a row that falls to zero
-- is removed. `refresh_analytics_link_click_rollup_hourly(keys)` rebuilds
-- whole hours (keys are `YYYY-MM-DDTHH:00:00Z`) from the ledger's own
-- `event_data`, so it does not depend on the link facts having been stamped
-- first; migration 011 queues every hour that holds a link click. The user
-- is a dimension so `report_link_click_hourly` can apply the deleted-user
-- filter unchanged; an absent page or target is stored as ''.
CREATE OR REPLACE FUNCTION analytics_link_click_rollup_upsert_sql(source TEXT)
RETURNS TEXT LANGUAGE sql IMMUTABLE AS $$
SELECT format('
    INSERT INTO analytics_link_click_rollup_hourly AS r (bucket, user_id, endpoint, target_url, is_external, clicks)
    SELECT date_trunc(''hour'', s.timestamp, ''UTC''), s.user_id, COALESCE(s.endpoint, ''''),
           COALESCE(s.event_data->>''target_url'', ''''),
           COALESCE(s.event_data->>''is_external'', ''false'') = ''true'', SUM(s.sign)
    FROM (%s) s
    WHERE s.event_type = ''link_click''
    GROUP BY 1, 2, 3, 4, 5
    ORDER BY 1, 2, 3, 4, 5
    ON CONFLICT (bucket, user_id, endpoint, target_url, is_external) DO UPDATE
    SET clicks = r.clicks + EXCLUDED.clicks
', source)
$$;

CREATE OR REPLACE FUNCTION refresh_analytics_link_click_rollup_hourly(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE hours TIMESTAMPTZ[]; written BIGINT;
BEGIN
    hours := ARRAY(SELECT DISTINCT date_trunc('hour', k::timestamptz, 'UTC') FROM unnest(keys) k);
    DELETE FROM analytics_link_click_rollup_hourly WHERE bucket = ANY(hours);
    EXECUTE analytics_link_click_rollup_upsert_sql(
        'SELECT e.*, 1 AS sign FROM analytics_events e JOIN unnest($1) h(b) '
        'ON e.timestamp >= h.b AND e.timestamp < h.b + interval ''1 hour''')
    USING hours;
    GET DIAGNOSTICS written = ROW_COUNT;
    RETURN written;
END $$;

CREATE OR REPLACE FUNCTION analytics_link_click_rollup_apply()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        EXECUTE analytics_link_click_rollup_upsert_sql('SELECT n.*, 1 AS sign FROM new_rows n');
        RETURN NULL;
    END IF;
    EXECUTE analytics_link_click_rollup_upsert_sql('SELECT o.*, -1 AS sign FROM old_rows o');
    DELETE FROM analytics_link_click_rollup_hourly
    WHERE clicks = 0
      AND bucket = ANY(ARRAY(SELECT DISTINCT date_trunc('hour', o.timestamp, 'UTC') FROM old_rows o));
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS analytics_link_click_rollup_insert ON analytics_events;
CREATE TRIGGER analytics_link_click_rollup_insert
    AFTER INSERT ON analytics_events
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION analytics_link_click_rollup_apply();

DROP TRIGGER IF EXISTS analytics_link_click_rollup_delete ON analytics_events;
CREATE TRIGGER analytics_link_click_rollup_delete
    AFTER DELETE ON analytics_events
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION analytics_link_click_rollup_apply();
