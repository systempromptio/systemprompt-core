-- The reporting projection is retired: its capture triggers and source views
-- on users and user_sessions go, and so do the privacy routines that fenced a
-- user deletion against the projection. Session expiry keeps its behaviour
-- under a name that no longer points at the projection (expire_user_sessions,
-- declared in users.sql).
DROP TRIGGER IF EXISTS reporting_capture_insert ON users;
DROP TRIGGER IF EXISTS reporting_capture_update ON users;
DROP TRIGGER IF EXISTS reporting_capture_delete ON users;
DROP TRIGGER IF EXISTS reporting_capture ON users;
DROP VIEW IF EXISTS reporting_source_users;
DROP TRIGGER IF EXISTS reporting_capture_insert ON user_sessions;
DROP TRIGGER IF EXISTS reporting_capture_update ON user_sessions;
DROP TRIGGER IF EXISTS reporting_capture_delete ON user_sessions;
DROP TRIGGER IF EXISTS reporting_capture ON user_sessions;
DROP VIEW IF EXISTS reporting_source_user_sessions;

DROP FUNCTION IF EXISTS public.expire_reporting_sessions(TIMESTAMPTZ);
DROP FUNCTION IF EXISTS public.reporting_session_is_retained(TEXT);
DROP FUNCTION IF EXISTS public.lock_users_reporting_sources();
DROP FUNCTION IF EXISTS public.finish_user_privacy();
DROP FUNCTION IF EXISTS public.begin_user_privacy();
DROP FUNCTION IF EXISTS public.reporting_user_is_retained(TEXT);
DROP FUNCTION IF EXISTS public.lock_user_deletion_for_retention();
