CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) NOT NULL UNIQUE CONSTRAINT users_email_normalised CHECK (email = lower(trim(email))),
    full_name VARCHAR(255),
    display_name VARCHAR(255),
    status TEXT NOT NULL CHECK(status IN ('active', 'inactive', 'suspended', 'pending', 'deleted', 'temporary')) DEFAULT 'active',
    email_verified BOOLEAN NOT NULL DEFAULT false,
    roles TEXT[] NOT NULL DEFAULT ARRAY['user']::TEXT[],
    is_bot BOOLEAN NOT NULL DEFAULT false,
    is_scanner BOOLEAN NOT NULL DEFAULT false,
    avatar_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_users_name ON users(name);
CREATE INDEX IF NOT EXISTS idx_users_bot_status ON users(is_bot, is_scanner);

-- Session retention for installations that expire idle sessions from their
-- own retention job: only sessions that can no longer be resumed go.
CREATE OR REPLACE FUNCTION public.expire_user_sessions(retained_after TIMESTAMPTZ)
RETURNS BIGINT LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
DECLARE removed BIGINT;
BEGIN
    IF retained_after IS NULL OR retained_after > NOW() THEN
        RAISE EXCEPTION 'Invalid session retention cutoff' USING ERRCODE = '22023';
    END IF;
    DELETE FROM public.user_sessions WHERE last_activity_at < retained_after
        AND (ended_at IS NOT NULL OR revoked_at IS NOT NULL OR expires_at IS NULL OR expires_at <= NOW());
    GET DIAGNOSTICS removed = ROW_COUNT;
    RETURN removed;
END
$$;
