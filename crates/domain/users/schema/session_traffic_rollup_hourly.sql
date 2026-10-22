-- Session traffic classes, counted at write time (store, don't compute).
--
--   session_traffic_rollup_hourly  sessions per hour bucket and traffic class
--                                  ('human', 'ghost', 'bot'), excluding the
--                                  sessions of users whose status is
--                                  'deleted', exactly as the report views do
--
-- `session_traffic_class` is the one rule. Statement triggers on
-- user_sessions move a count between classes when a session is inserted,
-- deleted or reclassified, and a statement trigger on users moves a user's
-- sessions out of (or back into) the counts when its status crosses
-- 'deleted'. `session_traffic_rollup_rebuild()` recomputes the whole table
-- from the ledger and is what migration 027 runs on an upgraded database.
CREATE TABLE IF NOT EXISTS session_traffic_rollup_hourly (
    bucket TIMESTAMPTZ NOT NULL,
    traffic_class TEXT NOT NULL CHECK (traffic_class IN ('human', 'ghost', 'bot')),
    sessions BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (bucket, traffic_class)
);

COMMENT ON TABLE session_traffic_rollup_hourly IS 'Sessions per hour and traffic class (human, ghost, bot) for sessions whose user is not deleted; maintained by statement triggers on user_sessions and users';

CREATE OR REPLACE FUNCTION session_traffic_class(
    is_bot BOOLEAN,
    is_ai_crawler BOOLEAN,
    is_scanner BOOLEAN,
    is_behavioral_bot BOOLEAN,
    landing_page TEXT,
    request_count INTEGER
) RETURNS TEXT
LANGUAGE sql IMMUTABLE PARALLEL SAFE
AS $$
    SELECT CASE
        WHEN NOT is_human_traffic(is_bot, is_ai_crawler, is_scanner, is_behavioral_bot) THEN 'bot'
        WHEN landing_page IS NOT NULL AND request_count > 0 THEN 'human'
        ELSE 'ghost'
    END
$$;

CREATE OR REPLACE FUNCTION session_traffic_rollup_rebuild()
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE written BIGINT;
BEGIN
    -- The triggers' upserts wait on this lock, so every session committed
    -- before the recount is counted exactly once and every later one by its
    -- own trigger afterwards.
    LOCK TABLE session_traffic_rollup_hourly IN SHARE ROW EXCLUSIVE MODE;
    DELETE FROM session_traffic_rollup_hourly;
    INSERT INTO session_traffic_rollup_hourly (bucket, traffic_class, sessions)
    SELECT date_trunc('hour', s.started_at),
           session_traffic_class(s.is_bot, s.is_ai_crawler, s.is_scanner, s.is_behavioral_bot, s.landing_page, s.request_count),
           COUNT(*)
    FROM user_sessions s
    WHERE NOT EXISTS (SELECT 1 FROM users d WHERE d.id = s.user_id AND d.status = 'deleted')
    GROUP BY 1, 2;
    GET DIAGNOSTICS written = ROW_COUNT;
    RETURN written;
END $$;

CREATE OR REPLACE FUNCTION session_traffic_rollup_inserted()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO session_traffic_rollup_hourly (bucket, traffic_class, sessions)
    SELECT date_trunc('hour', n.started_at),
           session_traffic_class(n.is_bot, n.is_ai_crawler, n.is_scanner, n.is_behavioral_bot, n.landing_page, n.request_count),
           COUNT(*)
    FROM new_rows n
    WHERE NOT EXISTS (SELECT 1 FROM users d WHERE d.id = n.user_id AND d.status = 'deleted')
    GROUP BY 1, 2
    ON CONFLICT (bucket, traffic_class) DO UPDATE
        SET sessions = session_traffic_rollup_hourly.sessions + EXCLUDED.sessions;
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION session_traffic_rollup_deleted()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO session_traffic_rollup_hourly (bucket, traffic_class, sessions)
    SELECT date_trunc('hour', o.started_at),
           session_traffic_class(o.is_bot, o.is_ai_crawler, o.is_scanner, o.is_behavioral_bot, o.landing_page, o.request_count),
           -COUNT(*)
    FROM old_rows o
    WHERE NOT EXISTS (SELECT 1 FROM users d WHERE d.id = o.user_id AND d.status = 'deleted')
    GROUP BY 1, 2
    ON CONFLICT (bucket, traffic_class) DO UPDATE
        SET sessions = session_traffic_rollup_hourly.sessions + EXCLUDED.sessions;
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION session_traffic_rollup_updated()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    WITH moved AS (
        SELECT o.started_at AS old_started_at, n.started_at AS new_started_at,
               o.user_id AS old_user_id, n.user_id AS new_user_id,
               session_traffic_class(o.is_bot, o.is_ai_crawler, o.is_scanner, o.is_behavioral_bot, o.landing_page, o.request_count) AS old_class,
               session_traffic_class(n.is_bot, n.is_ai_crawler, n.is_scanner, n.is_behavioral_bot, n.landing_page, n.request_count) AS new_class
        FROM new_rows n
        JOIN old_rows o ON o.session_id = n.session_id
    ),
    changed AS (
        SELECT * FROM moved
        WHERE (date_trunc('hour', old_started_at), old_class, old_user_id)
              IS DISTINCT FROM (date_trunc('hour', new_started_at), new_class, new_user_id)
    ),
    deltas AS (
        SELECT date_trunc('hour', c.old_started_at) AS bucket, c.old_class AS traffic_class, -1::BIGINT AS delta
        FROM changed c
        WHERE NOT EXISTS (SELECT 1 FROM users d WHERE d.id = c.old_user_id AND d.status = 'deleted')
        UNION ALL
        SELECT date_trunc('hour', c.new_started_at), c.new_class, 1::BIGINT
        FROM changed c
        WHERE NOT EXISTS (SELECT 1 FROM users d WHERE d.id = c.new_user_id AND d.status = 'deleted')
    )
    INSERT INTO session_traffic_rollup_hourly (bucket, traffic_class, sessions)
    SELECT bucket, traffic_class, SUM(delta) FROM deltas GROUP BY 1, 2
    ON CONFLICT (bucket, traffic_class) DO UPDATE
        SET sessions = session_traffic_rollup_hourly.sessions + EXCLUDED.sessions;
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION session_traffic_rollup_user_status_changed()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO session_traffic_rollup_hourly (bucket, traffic_class, sessions)
    SELECT date_trunc('hour', s.started_at),
           session_traffic_class(s.is_bot, s.is_ai_crawler, s.is_scanner, s.is_behavioral_bot, s.landing_page, s.request_count),
           SUM(CASE WHEN n.status = 'deleted' THEN -1 ELSE 1 END)
    FROM new_rows n
    JOIN old_rows o ON o.id = n.id
    JOIN user_sessions s ON s.user_id = n.id
    WHERE (o.status = 'deleted') IS DISTINCT FROM (n.status = 'deleted')
    GROUP BY 1, 2
    ON CONFLICT (bucket, traffic_class) DO UPDATE
        SET sessions = session_traffic_rollup_hourly.sessions + EXCLUDED.sessions;
    RETURN NULL;
END $$;

-- A hard delete of a user already in status 'deleted' cascades to sessions
-- the counts no longer hold; the cascade's delete trigger will subtract them
-- once more (the user row is gone by then), so they are added back first.
CREATE OR REPLACE FUNCTION session_traffic_rollup_deleted_user_removed()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.status = 'deleted' THEN
        INSERT INTO session_traffic_rollup_hourly (bucket, traffic_class, sessions)
        SELECT date_trunc('hour', s.started_at),
               session_traffic_class(s.is_bot, s.is_ai_crawler, s.is_scanner, s.is_behavioral_bot, s.landing_page, s.request_count),
               COUNT(*)
        FROM user_sessions s
        WHERE s.user_id = OLD.id
        GROUP BY 1, 2
        ON CONFLICT (bucket, traffic_class) DO UPDATE
            SET sessions = session_traffic_rollup_hourly.sessions + EXCLUDED.sessions;
    END IF;
    RETURN OLD;
END $$;

DROP TRIGGER IF EXISTS session_traffic_rollup_insert ON user_sessions;
CREATE TRIGGER session_traffic_rollup_insert
    AFTER INSERT ON user_sessions
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION session_traffic_rollup_inserted();

DROP TRIGGER IF EXISTS session_traffic_rollup_delete ON user_sessions;
CREATE TRIGGER session_traffic_rollup_delete
    AFTER DELETE ON user_sessions
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION session_traffic_rollup_deleted();

DROP TRIGGER IF EXISTS session_traffic_rollup_update ON user_sessions;
CREATE TRIGGER session_traffic_rollup_update
    AFTER UPDATE ON user_sessions
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION session_traffic_rollup_updated();

DROP TRIGGER IF EXISTS session_traffic_rollup_user_status ON users;
CREATE TRIGGER session_traffic_rollup_user_status
    AFTER UPDATE ON users
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION session_traffic_rollup_user_status_changed();

DROP TRIGGER IF EXISTS session_traffic_rollup_user_delete ON users;
CREATE TRIGGER session_traffic_rollup_user_delete
    BEFORE DELETE ON users
    FOR EACH ROW EXECUTE FUNCTION session_traffic_rollup_deleted_user_removed();
