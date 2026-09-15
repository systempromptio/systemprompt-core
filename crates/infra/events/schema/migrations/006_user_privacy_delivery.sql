CREATE OR REPLACE FUNCTION public.begin_user_reporting_outbox_delivery()
RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    LOCK TABLE public.event_outbox IN EXCLUSIVE MODE;
    PERFORM set_config('systemprompt.reporting_privacy', txid_current()::text, true);
    RETURN TRUE;
END
$$;
