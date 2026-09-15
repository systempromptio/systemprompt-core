CREATE SEQUENCE IF NOT EXISTS event_outbox_reporting_revision;

CREATE OR REPLACE FUNCTION sp_capture_reporting_change() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    source_row JSONB;
    previous_key TEXT;
    source_key TEXT;
    selected_row JSONB;
    change_revision BIGINT;
    event_id TEXT;
    deleted BOOLEAN;
BEGIN
    IF TG_OP = 'DELETE' THEN
        source_row := to_jsonb(OLD);
    ELSE
        source_row := to_jsonb(NEW);
    END IF;
    source_key := source_row ->> TG_ARGV[1];
    IF source_key IS NULL THEN
        RAISE EXCEPTION 'Reporting source % has no key %', TG_ARGV[0], TG_ARGV[1];
    END IF;
    IF TG_OP = 'UPDATE' THEN
        previous_key := to_jsonb(OLD) ->> TG_ARGV[1];
        IF previous_key IS DISTINCT FROM source_key THEN
            change_revision := nextval('event_outbox_reporting_revision');
            event_id := gen_random_uuid()::text; INSERT INTO event_outbox
                (id, channel, user_id, payload, actor_kind, actor_id, origin_instance_id, consumer, fact)
            VALUES (event_id, 'reporting', COALESCE(source_row ->> 'user_id', 'reporting-source'),
                '{}'::jsonb, 'job', 'reporting-capture', 'database', 'analytics_reporting',
                jsonb_build_object('consumer', 'analytics_reporting', 'kind', 'reporting.row', 'version', 1,
                    'data', jsonb_build_object('source', TG_ARGV[0], 'key', previous_key,
                        'revision', change_revision, 'deleted', true, 'row', 'null'::jsonb)));
            PERFORM pg_notify('systemprompt_events', event_id);
        END IF;
    END IF;
    deleted := TG_OP = 'DELETE';
    SELECT COALESCE(jsonb_object_agg(key, value), '{}'::jsonb)
        INTO selected_row FROM jsonb_each(source_row)
        WHERE key = ANY(string_to_array(TG_ARGV[2], ','));
    IF TG_OP = 'UPDATE' AND previous_key = source_key THEN
        IF selected_row = (
            SELECT COALESCE(jsonb_object_agg(key, value), '{}'::jsonb)
            FROM jsonb_each(to_jsonb(OLD)) WHERE key = ANY(string_to_array(TG_ARGV[2], ','))
        ) THEN
            RETURN NEW;
        END IF;
    END IF;
    change_revision := nextval('event_outbox_reporting_revision');
    event_id := gen_random_uuid()::text; INSERT INTO event_outbox
        (id, channel, user_id, payload, actor_kind, actor_id, origin_instance_id, consumer, fact)
    VALUES (event_id, 'reporting', COALESCE(source_row ->> 'user_id', 'reporting-source'),
        '{}'::jsonb, 'job', 'reporting-capture', 'database', 'analytics_reporting',
        jsonb_build_object('consumer', 'analytics_reporting', 'kind', 'reporting.row', 'version', 1,
            'data', jsonb_build_object('source', TG_ARGV[0], 'key', source_key,
                'revision', change_revision, 'deleted', deleted,
                'row', CASE WHEN deleted THEN 'null'::jsonb ELSE selected_row END)));
    PERFORM pg_notify('systemprompt_events', event_id);
    RETURN COALESCE(NEW, OLD);
END;
$$;
