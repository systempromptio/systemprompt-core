-- The working-set rows a read may admit: a row of a user-scoped fact counts
-- only under its owner's key. A row a topic audience added under an earlier
-- key rule stays unreachable until the `brain_fact_index` refresh rewrites
-- it. Every recall tier and the visible listing read this view, never the
-- table.
--
-- Replaced in place: CREATE OR REPLACE can append a column but cannot drop,
-- rename or reorder one; a change that does ships a migration that drops the
-- view first.

CREATE OR REPLACE VIEW v_brain_working_set AS
SELECT w.scope_key, w.fact_id, w.importance_base
FROM brain_working_set w
JOIN brain_facts f ON f.id = w.fact_id
WHERE f.scope_level <> 'user' OR w.scope_key = 'user:' || f.scope_key;
