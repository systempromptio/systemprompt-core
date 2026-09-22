-- Two funnel indexes are strict prefixes of a non-partial superset on the
-- same table, so each is pure write amplification. Neither is UNIQUE and
-- neither backs a constraint.

-- covered by idx_funnel_progress_unique (funnel_id, session_id)
DROP INDEX IF EXISTS idx_funnel_progress_funnel_id;
-- covered by funnel_steps_pkey (funnel_id, step_order)
DROP INDEX IF EXISTS idx_funnel_steps_funnel_id;
