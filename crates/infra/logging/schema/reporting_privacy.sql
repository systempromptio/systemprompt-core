CREATE OR REPLACE FUNCTION public.lock_logging_reporting_sources()
RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    LOCK TABLE analytics_events IN SHARE MODE;
    RETURN TRUE;
END
$$;
