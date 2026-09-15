ALTER TABLE analytics_projection_state ADD COLUMN IF NOT EXISTS evidence_cutoff TIMESTAMPTZ;
CREATE OR REPLACE FUNCTION public.reporting_row_retained(source_name TEXT, source_row JSONB)
RETURNS BOOLEAN LANGUAGE plpgsql STABLE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
DECLARE cutoff TIMESTAMPTZ; occurred TIMESTAMPTZ;
BEGIN
    IF source_name = 'markdown_content' THEN RETURN TRUE; END IF;
    IF source_name = 'users' THEN
        RETURN source_row->>'status' <> 'deleted'
            AND public.reporting_user_is_retained(source_row->>'id');
    END IF;
    IF NOT public.reporting_user_is_retained(source_row->>'user_id') THEN RETURN FALSE; END IF;
    SELECT evidence_cutoff INTO cutoff FROM public.analytics_projection_state WHERE singleton;
    occurred := CASE source_name
        WHEN 'user_sessions' THEN (source_row->>'last_activity_at')::timestamptz
        WHEN 'logs' THEN (source_row->>'timestamp')::timestamptz
        WHEN 'analytics_events' THEN (source_row->>'timestamp')::timestamptz
        ELSE (source_row->>'created_at')::timestamptz END;
    IF cutoff IS NOT NULL AND (occurred IS NULL OR occurred < cutoff) THEN RETURN FALSE; END IF;
    IF source_name IN ('agent_tasks', 'task_messages') THEN
        RETURN public.reporting_task_is_retained(source_row->>'task_id', cutoff);
    ELSIF source_name = 'ai_request_messages' THEN
        RETURN public.reporting_request_is_retained(source_row->>'request_id', cutoff);
    END IF;
    IF source_name = 'logs' AND source_row->>'user_id' IS NULL THEN
        RETURN public.reporting_session_is_retained(source_row->>'session_id');
    END IF;
    RETURN TRUE;
END
$$;

CREATE OR REPLACE FUNCTION public.prepare_reporting_privacy()
RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    PERFORM public.lock_user_deletion_for_retention();
    IF current_setting('systemprompt.reporting_privacy', true) = txid_current()::text THEN RETURN TRUE; END IF;
    PERFORM public.lock_users_reporting_sources();
    PERFORM public.lock_agent_reporting_sources();
    PERFORM public.lock_ai_reporting_sources();
    PERFORM public.lock_mcp_reporting_sources();
    PERFORM public.lock_content_reporting_sources();
    PERFORM public.lock_logging_reporting_sources();
    INSERT INTO public.analytics_projection_state(singleton) VALUES(TRUE) ON CONFLICT DO NOTHING;
    PERFORM set_config('systemprompt.reporting_privacy_sources', txid_current()::text, true);
    IF NOT COALESCE((SELECT initialized FROM public.analytics_projection_state WHERE singleton), FALSE) THEN
        RETURN FALSE;
    END IF;
    PERFORM pg_advisory_xact_lock(6003370107643648340);
    PERFORM public.begin_reporting_outbox_privacy();
    RETURN TRUE;
END
$$;

CREATE OR REPLACE FUNCTION public.apply_reporting_privacy_row(fact JSONB)
RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
DECLARE source_name TEXT := fact->>'source'; target_name TEXT; key_name TEXT;
    key_type TEXT; columns TEXT[]; assignments TEXT; source_row JSONB := fact->'row';
BEGIN
    CASE source_name
        WHEN 'users' THEN target_name := 'analytics_report_users'; key_name := 'id'; key_type := 'TEXT'; columns := ARRAY['id','name','status','roles','created_at'];
        WHEN 'user_sessions' THEN target_name := 'analytics_report_user_sessions'; key_name := 'session_id'; key_type := 'TEXT'; columns := ARRAY['session_id','user_id','started_at','last_activity_at','ended_at','duration_seconds','user_type','converted_at','expires_at','client_id','client_type','request_count','avg_response_time_ms','success_rate','error_count','task_count','message_count','ai_request_count','total_tokens_used','total_ai_cost_microdollars','ip_address','user_agent','device_type','browser','os','country','region','city','preferred_locale','referrer_source','referrer_url','landing_page','entry_url','utm_source','utm_medium','utm_campaign','utm_content','utm_term','endpoints_accessed','fingerprint_hash','is_bot','is_ai_crawler','is_scanner','is_behavioral_bot','behavioral_bot_reason','behavioral_bot_score','session_source','revoked_at'];
        WHEN 'agent_tasks' THEN target_name := 'analytics_report_agent_tasks'; key_name := 'task_id'; key_type := 'TEXT'; columns := ARRAY['task_id','context_id','status','status_timestamp','user_id','session_id','trace_id','agent_name','started_at','completed_at','execution_time_ms','error_message','version','created_at','updated_at'];
        WHEN 'task_messages' THEN target_name := 'analytics_report_task_messages'; key_name := 'id'; key_type := 'INTEGER'; columns := ARRAY['id','task_id','created_at'];
        WHEN 'user_contexts' THEN target_name := 'analytics_report_user_contexts'; key_name := 'context_id'; key_type := 'TEXT'; columns := ARRAY['context_id','user_id','session_id','name','kind','created_at','updated_at'];
        WHEN 'ai_requests' THEN target_name := 'analytics_report_ai_requests'; key_name := 'id'; key_type := 'TEXT'; columns := ARRAY['id','request_id','user_id','session_id','task_id','context_id','gateway_conversation_id','client_session_id','provider_request_id','trace_id','mcp_execution_id','provider','model','requested_model','route_match','temperature','top_p','max_tokens','tokens_used','input_tokens','output_tokens','cost_microdollars','latency_ms','upstream_latency_ms','cache_hit','cache_read_tokens','cache_creation_tokens','reasoning_tokens','is_streaming','status','error_message','actor_kind','actor_id','synthetic','request_kind','instance_id','created_at','updated_at','completed_at'];
        WHEN 'ai_request_messages' THEN target_name := 'analytics_report_ai_request_messages'; key_name := 'id'; key_type := 'TEXT'; columns := ARRAY['id','request_id','created_at'];
        WHEN 'mcp_tool_executions' THEN target_name := 'analytics_report_mcp_tool_executions'; key_name := 'mcp_execution_id'; key_type := 'TEXT'; columns := ARRAY['mcp_execution_id','tool_name','server_name','started_at','completed_at','execution_time_ms','status','error_message','user_id','session_id','context_id','task_id','trace_id','request_method','request_source','actor_kind','actor_id','ai_tool_call_id','created_at'];
        WHEN 'markdown_content' THEN target_name := 'analytics_report_markdown_content'; key_name := 'id'; key_type := 'TEXT'; columns := ARRAY['id','slug','title','source_id'];
        WHEN 'logs' THEN target_name := 'analytics_report_logs'; key_name := 'id'; key_type := 'TEXT'; columns := ARRAY['id','timestamp','level','module','message','user_id','session_id','task_id'];
        WHEN 'analytics_events' THEN target_name := 'analytics_report_analytics_events'; key_name := 'id'; key_type := 'TEXT'; columns := ARRAY['id','user_id','session_id','context_id','gateway_conversation_id','provider_request_id','event_type','event_category','severity','endpoint','error_code','response_time_ms','agent_id','task_id','message','metadata','event_data','timestamp'];
        ELSE RAISE EXCEPTION 'Unknown reporting source' USING ERRCODE = '22023';
    END CASE;
    IF COALESCE(fact->>'key', '') = '' OR COALESCE((fact->>'revision')::bigint, -1) < 0
        OR jsonb_typeof(fact->'deleted') IS DISTINCT FROM 'boolean' THEN
        RAISE EXCEPTION 'Invalid reporting key or revision' USING ERRCODE = '22023';
    END IF;
    IF (fact->>'deleted')::boolean THEN
        IF source_row IS DISTINCT FROM 'null'::jsonb THEN
            RAISE EXCEPTION 'Deleted reporting row must be null' USING ERRCODE = '22023';
        END IF;
        EXECUTE format('DELETE FROM public.%I WHERE %I = $1::%s', target_name, key_name, key_type) USING fact->>'key';
        RETURN TRUE;
    END IF;
    IF jsonb_typeof(source_row) <> 'object' OR source_row->>key_name IS DISTINCT FROM fact->>'key'
        OR (SELECT array_agg(k ORDER BY k) FROM jsonb_object_keys(source_row) k)
            IS DISTINCT FROM (SELECT array_agg(c ORDER BY c) FROM unnest(columns) c) THEN
        RAISE EXCEPTION 'Reporting row violates versioned contract' USING ERRCODE = '22023';
    END IF;
    IF NOT public.reporting_row_retained(source_name, source_row) THEN
        EXECUTE format('DELETE FROM public.%I WHERE %I = $1::%s', target_name, key_name, key_type) USING fact->>'key';
        RETURN FALSE;
    END IF;
    SELECT string_agg(format('%I = EXCLUDED.%I', c, c), ', ') INTO assignments FROM unnest(columns) c WHERE c <> key_name;
    EXECUTE format('INSERT INTO public.%I SELECT * FROM jsonb_populate_record(NULL::public.%I, $1) ON CONFLICT (%I) DO UPDATE SET %s',
        target_name, target_name, key_name, assignments) USING source_row;
    RETURN TRUE;
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
    IF NOT COALESCE((SELECT initialized FROM public.analytics_projection_state WHERE singleton), FALSE) THEN
        IF retained_after > NOW() OR retained_after < (SELECT evidence_cutoff FROM public.analytics_projection_state WHERE singleton) THEN
            RAISE EXCEPTION 'Invalid reporting privacy cutoff' USING ERRCODE = '22023';
        END IF;
        UPDATE public.analytics_projection_state SET evidence_cutoff = COALESCE(retained_after, evidence_cutoff) WHERE singleton;
        RETURN 0;
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
        generation = generation + 1 WHERE singleton;
    DELETE FROM public.analytics_projection_revisions;
    RETURN count_processed;
END
$$;
