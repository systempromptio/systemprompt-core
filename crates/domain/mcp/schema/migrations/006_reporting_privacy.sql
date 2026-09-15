CREATE OR REPLACE FUNCTION public.lock_mcp_reporting_sources()
RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    LOCK TABLE mcp_tool_executions IN SHARE MODE;
    RETURN TRUE;
END
$$;
