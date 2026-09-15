CREATE TABLE IF NOT EXISTS user_device_certs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    fingerprint VARCHAR(128) NOT NULL UNIQUE,
    label VARCHAR(100) NOT NULL,
    enrolled_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    revoked_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_user_device_certs_user ON user_device_certs(user_id);
CREATE INDEX IF NOT EXISTS idx_user_device_certs_fingerprint_active
    ON user_device_certs(fingerprint)
    WHERE revoked_at IS NULL;

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
