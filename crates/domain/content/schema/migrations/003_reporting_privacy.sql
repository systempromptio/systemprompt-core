CREATE OR REPLACE FUNCTION public.lock_content_reporting_sources()
RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    LOCK TABLE markdown_content IN SHARE MODE;
    RETURN TRUE;
END
$$;
