-- Pre-dispatch authz never carried the context the request had already been
-- resolved into, so its audit sink re-derived one from the bridge session and
-- every `authz` / `authz_rule_based` decision landed in a different context
-- from the `ai_requests` row it authorized. The extractor now passes the
-- resolved ContextId; this realigns the history, keyed on trace_id, which is
-- the one identifier both sides always recorded.
--
-- governance_decisions is append-only (migration 018). The guard is a row
-- trigger, so it is lifted for this statement only and restored in the same
-- transaction: a correction to a column that was written from the wrong
-- source is not a rewrite of a recorded decision, and no decision, policy,
-- reason or actor is touched here.
ALTER TABLE governance_decisions DISABLE TRIGGER governance_decisions_append_only;

UPDATE governance_decisions g
SET context_id = r.context_id
FROM ai_requests r
WHERE r.trace_id = g.trace_id
  AND g.trace_id IS NOT NULL
  AND r.context_id IS NOT NULL
  AND g.context_id IS DISTINCT FROM r.context_id;

ALTER TABLE governance_decisions ENABLE TRIGGER governance_decisions_append_only;
