CREATE OR REPLACE FUNCTION public.prepare_reporting_privacy()
RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    PERFORM public.lock_user_deletion_for_retention();
    IF current_setting('systemprompt.reporting_privacy', true) = txid_current()::text THEN
        RETURN COALESCE((SELECT initialized FROM public.analytics_projection_state WHERE singleton), FALSE);
    END IF;
    PERFORM public.lock_users_reporting_sources();
    PERFORM public.lock_agent_reporting_sources();
    PERFORM public.lock_ai_reporting_sources();
    PERFORM public.lock_mcp_reporting_sources();
    PERFORM public.lock_content_reporting_sources();
    PERFORM public.lock_logging_reporting_sources();
    INSERT INTO public.analytics_projection_state(singleton) VALUES(TRUE) ON CONFLICT DO NOTHING;
    PERFORM set_config('systemprompt.reporting_privacy_sources', txid_current()::text, true);
    PERFORM pg_advisory_xact_lock(6003370107643648340);
    PERFORM public.begin_reporting_outbox_privacy();
    RETURN COALESCE((SELECT initialized FROM public.analytics_projection_state WHERE singleton), FALSE);
END
$$;

CREATE OR REPLACE FUNCTION public.finish_reporting_privacy(retained_after TIMESTAMPTZ)
RETURNS BIGINT LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
DECLARE item RECORD; count_processed BIGINT := 0; revision_cutoff BIGINT; previous_cutoff TIMESTAMPTZ;
BEGIN
    IF current_setting('systemprompt.reporting_privacy_sources', true) IS DISTINCT FROM txid_current()::text THEN
        RAISE EXCEPTION 'Reporting privacy sources were not fenced' USING ERRCODE = '55000';
    END IF;
    IF current_setting('systemprompt.reporting_privacy', true) IS DISTINCT FROM txid_current()::text THEN
        RAISE EXCEPTION 'Reporting privacy was not prepared' USING ERRCODE = '55000';
    END IF;
    SELECT evidence_cutoff INTO previous_cutoff FROM public.analytics_projection_state WHERE singleton FOR UPDATE;
    IF retained_after > NOW() OR retained_after < previous_cutoff THEN
        RAISE EXCEPTION 'Invalid reporting privacy cutoff' USING ERRCODE = '22023';
    END IF;
    UPDATE public.analytics_projection_state SET evidence_cutoff = COALESCE(retained_after, evidence_cutoff) WHERE singleton;
    FOR item IN SELECT * FROM public.reporting_privacy_changes() LOOP
        count_processed := count_processed + 1;
        IF count_processed > 10000 THEN
            RAISE EXCEPTION 'Reporting privacy change bound exceeded' USING ERRCODE = '54000';
        END IF;
        IF item.envelope->>'consumer' IS DISTINCT FROM 'analytics_reporting' OR item.envelope->>'kind' IS DISTINCT FROM 'reporting.row'
            OR item.envelope->>'version' IS DISTINCT FROM '1' THEN
            RAISE EXCEPTION 'Invalid reporting privacy envelope' USING ERRCODE = '22023';
        END IF;
        PERFORM public.apply_reporting_privacy_row(item.envelope->'data');
        PERFORM public.acknowledge_reporting_privacy(item.event_id);
    END LOOP;
    DELETE FROM public.analytics_report_users AS retained_row WHERE NOT public.reporting_row_retained('users', to_jsonb(retained_row));
    DELETE FROM public.analytics_report_user_sessions AS retained_row WHERE NOT public.reporting_row_retained('user_sessions', to_jsonb(retained_row));
    DELETE FROM public.analytics_report_agent_tasks AS retained_row WHERE NOT public.reporting_row_retained('agent_tasks', to_jsonb(retained_row));
    DELETE FROM public.analytics_report_task_messages AS retained_row WHERE NOT public.reporting_row_retained('task_messages', to_jsonb(retained_row));
    DELETE FROM public.analytics_report_user_contexts AS retained_row WHERE NOT public.reporting_row_retained('user_contexts', to_jsonb(retained_row));
    DELETE FROM public.analytics_report_ai_requests AS retained_row WHERE NOT public.reporting_row_retained('ai_requests', to_jsonb(retained_row));
    DELETE FROM public.analytics_report_ai_request_messages AS retained_row WHERE NOT public.reporting_row_retained('ai_request_messages', to_jsonb(retained_row));
    DELETE FROM public.analytics_report_mcp_tool_executions AS retained_row WHERE NOT public.reporting_row_retained('mcp_tool_executions', to_jsonb(retained_row));
    DELETE FROM public.analytics_report_logs AS retained_row WHERE NOT public.reporting_row_retained('logs', to_jsonb(retained_row));
    DELETE FROM public.analytics_report_analytics_events AS retained_row WHERE NOT public.reporting_row_retained('analytics_events', to_jsonb(retained_row));
    revision_cutoff := public.finish_reporting_outbox_privacy();
    UPDATE public.analytics_projection_state SET cutoff_revision = GREATEST(cutoff_revision, revision_cutoff),
        generation = generation + CASE WHEN initialized THEN 1 ELSE 0 END WHERE singleton;
    DELETE FROM public.analytics_projection_revisions;
    RETURN count_processed;
END
$$;

CREATE OR REPLACE FUNCTION public.finish_reporting_compaction(requested_after TIMESTAMPTZ)
RETURNS BIGINT LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
DECLARE effective_cutoff TIMESTAMPTZ;
BEGIN
    IF requested_after IS NULL OR requested_after > NOW() THEN
        RAISE EXCEPTION 'Invalid reporting compaction cutoff' USING ERRCODE = '22023';
    END IF;
    SELECT GREATEST(evidence_cutoff, requested_after) INTO effective_cutoff
        FROM public.analytics_projection_state WHERE singleton FOR UPDATE;
    RETURN public.finish_reporting_privacy(effective_cutoff);
END
$$;
