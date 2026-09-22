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

CREATE OR REPLACE FUNCTION public.lock_user_deletion_for_retention()
RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    LOCK TABLE public.users IN SHARE ROW EXCLUSIVE MODE;
    RETURN TRUE;
END
$$;

CREATE OR REPLACE FUNCTION public.reporting_user_is_retained(subject TEXT)
RETURNS BOOLEAN LANGUAGE sql STABLE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
    SELECT subject IS NULL OR EXISTS (
        SELECT 1 FROM public.users WHERE id = subject AND status <> 'deleted'
    )
$$;

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

CREATE OR REPLACE FUNCTION public.finish_user_privacy()
RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    IF to_regprocedure('public.finish_reporting_privacy(timestamp with time zone)') IS NOT NULL THEN
        PERFORM public.finish_reporting_privacy(NULL);
    END IF;
    RETURN TRUE;
END
$$;

CREATE OR REPLACE FUNCTION public.lock_users_reporting_sources()
RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    LOCK TABLE user_sessions IN SHARE MODE;
    RETURN TRUE;
END
$$;

CREATE OR REPLACE FUNCTION public.expire_reporting_sessions(retained_after TIMESTAMPTZ)
RETURNS BIGINT LANGUAGE plpgsql VOLATILE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
DECLARE removed BIGINT;
BEGIN
    IF retained_after IS NULL OR retained_after > NOW() THEN
        RAISE EXCEPTION 'Invalid session retention cutoff' USING ERRCODE = '22023';
    END IF;
    PERFORM public.prepare_user_reporting_privacy();
    DELETE FROM public.user_sessions WHERE last_activity_at < retained_after
        AND (ended_at IS NOT NULL OR revoked_at IS NOT NULL OR expires_at IS NULL OR expires_at <= NOW());
    GET DIAGNOSTICS removed = ROW_COUNT;
    RETURN removed;
END
$$;

CREATE OR REPLACE FUNCTION public.reporting_session_is_retained(subject TEXT)
RETURNS BOOLEAN LANGUAGE plpgsql STABLE SECURITY INVOKER
SET search_path = pg_catalog, public AS $$
BEGIN
    RETURN subject IS NULL OR EXISTS (
        SELECT 1 FROM public.user_sessions WHERE session_id = subject
            AND public.reporting_user_is_retained(user_id)
    );
END
$$;
