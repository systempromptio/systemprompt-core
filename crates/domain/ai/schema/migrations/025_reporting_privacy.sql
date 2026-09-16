CREATE OR REPLACE FUNCTION public.lock_ai_reporting_sources()
RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    LOCK TABLE ai_requests, ai_request_messages IN SHARE MODE;
    RETURN TRUE;
END
$$;

CREATE OR REPLACE FUNCTION public.reporting_request_is_retained(subject TEXT, retained_after TIMESTAMPTZ)
RETURNS BOOLEAN LANGUAGE plpgsql STABLE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1 FROM public.ai_requests WHERE id = subject
            AND (retained_after IS NULL OR created_at >= retained_after)
            AND public.reporting_user_is_retained(user_id)
    );
END
$$;
