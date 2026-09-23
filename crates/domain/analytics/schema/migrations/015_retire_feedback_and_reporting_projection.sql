-- The feedback fact/snapshot engine and the reporting projection are gone.
-- Nothing read the snapshots after the last consumer was deleted, and the
-- `analytics` CLI now reads views over the source tables, so every table the
-- two pipelines kept is dropped here along with the privacy routines that
-- guarded the projection. Views go before the tables they select from.
DROP VIEW IF EXISTS analytics_snapshot_contributions;
DROP VIEW IF EXISTS analytics_snapshot_dimensions;
DROP VIEW IF EXISTS analytics_report_v_bot_sessions;
DROP VIEW IF EXISTS analytics_report_v_engaged_traffic;
DROP VIEW IF EXISTS analytics_report_v_clean_traffic;

DROP TABLE IF EXISTS analytics_snapshot_jobs;
DROP TABLE IF EXISTS analytics_snapshot_state;
DROP TABLE IF EXISTS analytics_snapshot_identities;
DROP TABLE IF EXISTS analytics_snapshot_daily;
DROP TABLE IF EXISTS analytics_snapshot_shadow;
DROP TABLE IF EXISTS analytics_snapshot_dirty;
DROP TABLE IF EXISTS analytics_feedback_snapshots;
DROP TABLE IF EXISTS analytics_fact_backfill_pages;
DROP TABLE IF EXISTS analytics_fact_backfills;
DROP TABLE IF EXISTS analytics_fact_deltas;
DROP TABLE IF EXISTS analytics_fact_checkpoints;
DROP TABLE IF EXISTS analytics_fact_consumers;
DROP TABLE IF EXISTS analytics_normalized_facts;
DROP TABLE IF EXISTS analytics_fact_changes;
DROP TABLE IF EXISTS analytics_ingestion_producers;

DROP FUNCTION IF EXISTS public.finish_reporting_compaction(TIMESTAMPTZ);
DROP FUNCTION IF EXISTS public.finish_reporting_privacy(TIMESTAMPTZ);
DROP FUNCTION IF EXISTS public.prepare_user_reporting_privacy();
DROP FUNCTION IF EXISTS public.prepare_reporting_privacy();
DROP FUNCTION IF EXISTS public.deliver_reporting_privacy_backlog();
DROP FUNCTION IF EXISTS public.deliver_reporting_privacy_changes();
DROP FUNCTION IF EXISTS public.deliver_reporting_privacy_page();
DROP FUNCTION IF EXISTS public.apply_reporting_privacy_row(JSONB);
DROP FUNCTION IF EXISTS public.reporting_row_retained(TEXT, JSONB);

DROP TABLE IF EXISTS analytics_report_users;
DROP TABLE IF EXISTS analytics_report_user_sessions;
DROP TABLE IF EXISTS analytics_report_agent_tasks;
DROP TABLE IF EXISTS analytics_report_task_messages;
DROP TABLE IF EXISTS analytics_report_user_contexts;
DROP TABLE IF EXISTS analytics_report_ai_requests;
DROP TABLE IF EXISTS analytics_report_mcp_tool_executions;
DROP TABLE IF EXISTS analytics_report_markdown_content;
DROP TABLE IF EXISTS analytics_report_analytics_events;
DROP TABLE IF EXISTS analytics_projection_revisions;
DROP TABLE IF EXISTS analytics_projection_state;

-- Funnels and anomaly thresholds never had a caller outside their own
-- repositories.
DROP TABLE IF EXISTS funnel_progress;
DROP TABLE IF EXISTS funnel_steps;
DROP TABLE IF EXISTS funnels;
DROP TABLE IF EXISTS anomaly_thresholds;
