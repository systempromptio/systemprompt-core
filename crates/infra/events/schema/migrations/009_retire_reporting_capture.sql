-- @cost: rows=1154 measured=5ms triggers=live
-- The reporting projection is gone, and with it the `analytics_reporting`
-- outbox consumer. Its queued facts would never be claimed again, and the
-- relay prunes only processed or consumer-less rows, so they are deleted
-- here. The capture function goes with CASCADE: it takes every
-- `reporting_capture*` trigger still bound to a source table with it,
-- whichever extension's migration would otherwise have dropped it first.
DELETE FROM event_outbox WHERE consumer = 'analytics_reporting';

DROP FUNCTION IF EXISTS public.sp_capture_reporting_change() CASCADE;
DROP FUNCTION IF EXISTS public.sp_reporting_fact(TEXT, TEXT, BOOLEAN, JSONB);
DROP FUNCTION IF EXISTS public.sp_reporting_project(JSONB, TEXT[]);
DROP SEQUENCE IF EXISTS event_outbox_reporting_revision;

DROP FUNCTION IF EXISTS public.begin_reporting_outbox_privacy();
DROP FUNCTION IF EXISTS public.reporting_privacy_changes();
DROP FUNCTION IF EXISTS public.acknowledge_reporting_privacy(TEXT);
DROP FUNCTION IF EXISTS public.finish_reporting_outbox_privacy();
DROP FUNCTION IF EXISTS public.begin_user_reporting_outbox_delivery();
DROP FUNCTION IF EXISTS public.fence_reporting_outbox_claims();
