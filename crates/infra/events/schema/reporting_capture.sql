CREATE SEQUENCE IF NOT EXISTS event_outbox_reporting_revision;

-- One reporting fact envelope. The revision is minted here, once per emitted
-- row, so a statement that touches ten thousand rows still gives every fact
-- its own place in the projector's order.
CREATE OR REPLACE FUNCTION sp_reporting_fact(source_name TEXT, source_key TEXT, deleted BOOLEAN, selected_row JSONB)
RETURNS JSONB LANGUAGE plpgsql VOLATILE AS $$
BEGIN
    IF source_key IS NULL THEN
        RAISE EXCEPTION 'Reporting source % has a row without its key', source_name;
    END IF;
    RETURN jsonb_build_object('consumer', 'analytics_reporting', 'kind', 'reporting.row', 'version', 1,
        'data', jsonb_build_object('source', source_name, 'key', source_key,
            'revision', nextval('event_outbox_reporting_revision'), 'deleted', deleted,
            'row', CASE WHEN deleted THEN 'null'::jsonb ELSE selected_row END));
END;
$$;

CREATE OR REPLACE FUNCTION sp_reporting_project(source_row JSONB, columns TEXT[])
RETURNS JSONB LANGUAGE sql IMMUTABLE AS $$
    SELECT COALESCE(jsonb_object_agg(key, value), '{}'::jsonb)
    FROM jsonb_each(source_row) WHERE key = ANY(columns);
$$;

-- Statement-level capture. Bound once per operation with
-- REFERENCING OLD TABLE AS old_rows / NEW TABLE AS new_rows, so a bulk
-- insert, a retention delete or a settlement update costs one INSERT ... SELECT
-- into the outbox instead of one trigger invocation per row. Nothing is
-- notified: the reporting worker polls, and the SSE bridge never fans a
-- reporting row out.
--
-- The row-level branch stays: the installer replaces function bodies before
-- it runs migrations, and a database mid-upgrade still carries the row form
-- of `reporting_capture` on every source until the declarative phase swaps
-- it. A migration that touches a source table in that window fires the row
-- trigger into this body, and TG_LEVEL is what keeps it working.
-- Arguments: source table, key column, comma-separated projected columns.
CREATE OR REPLACE FUNCTION sp_capture_reporting_change() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    source_name TEXT := TG_ARGV[0];
    key_name TEXT := TG_ARGV[1];
    columns TEXT[] := string_to_array(TG_ARGV[2], ',');
    source_row JSONB;
    previous_key TEXT;
    selected_row JSONB;
BEGIN
    IF TG_LEVEL = 'ROW' THEN
        IF TG_OP = 'DELETE' THEN
            source_row := to_jsonb(OLD);
        ELSE
            source_row := to_jsonb(NEW);
        END IF;
        IF TG_OP = 'UPDATE' THEN
            previous_key := to_jsonb(OLD) ->> key_name;
            IF previous_key IS DISTINCT FROM (source_row ->> key_name) THEN
                INSERT INTO event_outbox
                    (id, channel, user_id, payload, actor_kind, actor_id, origin_instance_id, consumer, fact)
                VALUES (gen_random_uuid()::text, 'reporting', COALESCE(source_row ->> 'user_id', 'reporting-source'),
                    '{}'::jsonb, 'job', 'reporting-capture', 'database', 'analytics_reporting',
                    sp_reporting_fact(source_name, previous_key, true, NULL));
            END IF;
        END IF;
        selected_row := sp_reporting_project(source_row, columns);
        IF TG_OP = 'UPDATE' AND previous_key = (source_row ->> key_name)
            AND selected_row = sp_reporting_project(to_jsonb(OLD), columns) THEN
            RETURN NEW;
        END IF;
        INSERT INTO event_outbox
            (id, channel, user_id, payload, actor_kind, actor_id, origin_instance_id, consumer, fact)
        VALUES (gen_random_uuid()::text, 'reporting', COALESCE(source_row ->> 'user_id', 'reporting-source'),
            '{}'::jsonb, 'job', 'reporting-capture', 'database', 'analytics_reporting',
            sp_reporting_fact(source_name, source_row ->> key_name, TG_OP = 'DELETE', selected_row));
        RETURN COALESCE(NEW, OLD);
    END IF;
    IF TG_OP = 'INSERT' THEN
        INSERT INTO event_outbox
            (id, channel, user_id, payload, actor_kind, actor_id, origin_instance_id, consumer, fact)
        SELECT gen_random_uuid()::text, 'reporting', COALESCE(r.j ->> 'user_id', 'reporting-source'),
            '{}'::jsonb, 'job', 'reporting-capture', 'database', 'analytics_reporting',
            sp_reporting_fact(source_name, r.j ->> key_name, false, sp_reporting_project(r.j, columns))
        FROM (SELECT to_jsonb(n) AS j FROM new_rows n) r;
    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO event_outbox
            (id, channel, user_id, payload, actor_kind, actor_id, origin_instance_id, consumer, fact)
        SELECT gen_random_uuid()::text, 'reporting', COALESCE(r.j ->> 'user_id', 'reporting-source'),
            '{}'::jsonb, 'job', 'reporting-capture', 'database', 'analytics_reporting',
            sp_reporting_fact(source_name, r.j ->> key_name, true, NULL)
        FROM (SELECT to_jsonb(o) AS j FROM old_rows o) r;
    ELSE
        -- A key that left the statement is a tombstone; minted first so the
        -- rename's tombstone orders before the row that replaces it.
        INSERT INTO event_outbox
            (id, channel, user_id, payload, actor_kind, actor_id, origin_instance_id, consumer, fact)
        SELECT gen_random_uuid()::text, 'reporting', COALESCE(o.j ->> 'user_id', 'reporting-source'),
            '{}'::jsonb, 'job', 'reporting-capture', 'database', 'analytics_reporting',
            sp_reporting_fact(source_name, o.j ->> key_name, true, NULL)
        FROM (SELECT to_jsonb(o) AS j FROM old_rows o) o
        WHERE o.j ->> key_name IS NULL
            OR NOT EXISTS (SELECT 1 FROM new_rows n WHERE to_jsonb(n) ->> key_name = o.j ->> key_name);
        -- Rows whose projected columns changed, or whose key is new to the
        -- table; an update that only touched unprojected columns emits nothing.
        INSERT INTO event_outbox
            (id, channel, user_id, payload, actor_kind, actor_id, origin_instance_id, consumer, fact)
        SELECT gen_random_uuid()::text, 'reporting', COALESCE(n.j ->> 'user_id', 'reporting-source'),
            '{}'::jsonb, 'job', 'reporting-capture', 'database', 'analytics_reporting',
            sp_reporting_fact(source_name, n.j ->> key_name, false, n.selected)
        FROM (SELECT to_jsonb(n) AS j, sp_reporting_project(to_jsonb(n), columns) AS selected FROM new_rows n) n
        LEFT JOIN (SELECT to_jsonb(o) AS j, sp_reporting_project(to_jsonb(o), columns) AS selected FROM old_rows o) o
            ON o.j ->> key_name = n.j ->> key_name
        WHERE n.j ->> key_name IS NULL OR o.j IS NULL OR o.selected <> n.selected;
    END IF;
    RETURN NULL;
END;
$$;
