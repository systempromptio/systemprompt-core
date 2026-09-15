CREATE OR REPLACE FUNCTION public.begin_reporting_outbox_privacy()
RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    LOCK TABLE public.event_outbox IN SHARE ROW EXCLUSIVE MODE;
    IF EXISTS (SELECT 1 FROM public.event_outbox
        WHERE consumer = 'analytics_reporting' AND processed_at IS NULL) THEN
        RAISE EXCEPTION 'Reporting privacy waits for pending committed evidence' USING ERRCODE = '55000';
    END IF;
    PERFORM set_config('systemprompt.reporting_privacy', txid_current()::text, true);
    RETURN TRUE;
END
$$;

CREATE OR REPLACE FUNCTION public.reporting_privacy_changes()
RETURNS TABLE(event_id TEXT, envelope JSONB)
LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    IF current_setting('systemprompt.reporting_privacy', true) IS DISTINCT FROM txid_current()::text THEN
        RAISE EXCEPTION 'Reporting privacy was not prepared' USING ERRCODE = '55000';
    END IF;
    RETURN QUERY SELECT id, fact FROM public.event_outbox
        WHERE consumer = 'analytics_reporting' AND processed_at IS NULL
        ORDER BY (fact->'data'->>'revision')::bigint, id LIMIT 10001 FOR UPDATE;
END
$$;

CREATE OR REPLACE FUNCTION public.acknowledge_reporting_privacy(event_id TEXT)
RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    IF current_setting('systemprompt.reporting_privacy', true) IS DISTINCT FROM txid_current()::text THEN
        RAISE EXCEPTION 'Reporting privacy was not prepared' USING ERRCODE = '55000';
    END IF;
    UPDATE public.event_outbox SET processed_at = NOW()
        WHERE id = event_id AND consumer = 'analytics_reporting' AND processed_at IS NULL;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Reporting privacy acknowledgement lost its evidence' USING ERRCODE = '55000';
    END IF;
    RETURN TRUE;
END
$$;

CREATE OR REPLACE FUNCTION public.finish_reporting_outbox_privacy()
RETURNS BIGINT LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
DECLARE cutoff BIGINT;
BEGIN
    IF current_setting('systemprompt.reporting_privacy', true) IS DISTINCT FROM txid_current()::text THEN
        RAISE EXCEPTION 'Reporting privacy was not prepared' USING ERRCODE = '55000';
    END IF;
    IF EXISTS (SELECT 1 FROM public.event_outbox
        WHERE consumer = 'analytics_reporting' AND processed_at IS NULL) THEN
        RAISE EXCEPTION 'Reporting privacy cannot discard pending evidence' USING ERRCODE = '55000';
    END IF;
    cutoff := nextval('public.event_outbox_reporting_revision');
    DELETE FROM public.event_outbox WHERE consumer = 'analytics_reporting' AND processed_at IS NOT NULL;
    RETURN cutoff;
END
$$;
