-- Per project and client: how many recalls ran, how many injected, the
-- facts they injected and withheld by reason, the mean tokens an injecting
-- recall spent, and the recalls skipped by deadline, unavailability or an
-- empty result. `workflow_id` is the declared workflow profile the project
-- names, NULL when none is declared.
--
-- Replaced in place: CREATE OR REPLACE can append a column but cannot drop,
-- rename or reorder one; a change that does ships a migration that drops the
-- view first.

CREATE OR REPLACE VIEW v_brain_workflow_recall AS
SELECT
    r.project,
    w.id AS workflow_id,
    r.client,
    COUNT(*) AS recalls,
    COUNT(*) FILTER (WHERE r.skipped IS NULL) AS injecting_recalls,
    COALESCE(SUM(jsonb_array_length(r.injected)), 0) AS facts_injected,
    COALESCE(SUM((r.withheld->>'scope')::int), 0) AS withheld_scope,
    COALESCE(SUM((r.withheld->>'source')::int), 0) AS withheld_source,
    COALESCE(SUM((r.withheld->>'agent_capped')::int), 0) AS withheld_agent_capped,
    COALESCE(SUM((r.withheld->>'stale')::int), 0) AS withheld_stale,
    COALESCE(SUM((r.withheld->>'review_due')::int), 0) AS withheld_review_due,
    COALESCE(SUM((r.withheld->>'quota')::int), 0) AS withheld_quota,
    COALESCE(SUM((r.withheld->>'budget')::int), 0) AS withheld_budget,
    COALESCE(SUM((r.withheld->>'unavailable')::int), 0) AS withheld_unavailable,
    AVG(r.tokens) FILTER (WHERE r.skipped IS NULL)::float8 AS avg_tokens,
    COUNT(*) FILTER (WHERE r.skipped = 'deadline') AS deadline_hits,
    COUNT(*) FILTER (WHERE r.skipped = 'unavailable') AS unavailable,
    COUNT(*) FILTER (WHERE r.skipped = 'no_facts') AS no_facts,
    MIN(r.created_at) AS first_recall_at,
    MAX(r.created_at) AS last_recall_at
FROM brain_recalls r
LEFT JOIN brain_workflows w ON w.id = r.project
GROUP BY r.project, w.id, r.client;
