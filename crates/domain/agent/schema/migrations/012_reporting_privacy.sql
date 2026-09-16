CREATE OR REPLACE FUNCTION public.lock_agent_reporting_sources()
RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    LOCK TABLE agent_tasks, task_messages, user_contexts IN SHARE MODE;
    RETURN TRUE;
END
$$;

CREATE OR REPLACE FUNCTION public.reporting_task_is_retained(subject TEXT, retained_after TIMESTAMPTZ)
RETURNS BOOLEAN LANGUAGE plpgsql STABLE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1 FROM public.agent_tasks t JOIN public.user_contexts c USING(context_id)
        WHERE t.task_id = subject AND (retained_after IS NULL OR t.created_at >= retained_after)
            AND public.reporting_user_is_retained(t.user_id)
            AND public.reporting_user_is_retained(c.user_id)
    );
END
$$;
