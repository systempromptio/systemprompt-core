-- Public users-owned device eligibility interface. Locks last until caller transaction ends.
CREATE OR REPLACE FUNCTION public.active_device_identity(requested_device TEXT)
RETURNS TABLE(device_id TEXT, consumer_id TEXT)
LANGUAGE sql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public
AS $$
    SELECT d.id, d.user_id FROM public.user_device_certs d
    WHERE d.id = requested_device AND d.revoked_at IS NULL
    FOR SHARE OF d
$$;

-- A scalar consumer key prevents callers submitting an unbounded identity array.
CREATE OR REPLACE FUNCTION public.active_devices_for_consumer(requested_consumer TEXT)
RETURNS TABLE(device_id TEXT, consumer_id TEXT)
LANGUAGE sql STABLE SECURITY INVOKER
SET search_path = pg_catalog, public
AS $$
    SELECT d.id, d.user_id FROM public.user_device_certs d
    WHERE d.user_id = requested_consumer AND d.revoked_at IS NULL
$$;
