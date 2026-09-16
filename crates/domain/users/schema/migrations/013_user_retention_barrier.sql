-- Owner deletion must finish before retention acquires locks on dependent tables.
CREATE OR REPLACE FUNCTION public.lock_user_deletion_for_retention()
RETURNS BOOLEAN
LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public
AS $$
BEGIN
    LOCK TABLE public.users IN SHARE MODE;
    RETURN TRUE;
END
$$;
