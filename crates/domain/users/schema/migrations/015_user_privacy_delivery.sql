CREATE OR REPLACE FUNCTION public.begin_user_privacy()
RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    PERFORM public.lock_user_deletion_for_retention();
    IF to_regprocedure('public.prepare_user_reporting_privacy()') IS NOT NULL THEN
        PERFORM public.prepare_user_reporting_privacy();
    ELSIF to_regprocedure('public.prepare_reporting_privacy()') IS NOT NULL THEN
        PERFORM public.prepare_reporting_privacy();
    END IF;
    RETURN TRUE;
END
$$;
