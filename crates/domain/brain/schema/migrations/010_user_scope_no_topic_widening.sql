-- A user-scoped fact is reachable only through its owner's key: topic
-- audiences widen project, group and org facts, never a user's. The key
-- derivation is one function the per-fact refresh, the rebuild and the
-- consistency count share. Every user-scoped fact is queued for the
-- `brain_fact_index` projection, whose refresh rewrites the fact's
-- working-set rows through that function and so drops the rows a topic
-- audience added; the owner's own row is kept. Until it does, reads go
-- through `v_brain_working_set`, which admits a user-scoped fact only under
-- its owner's key.
-- @cost: rows=0 measured=0.0s triggers=live

CREATE OR REPLACE FUNCTION brain_own_scope_key(p_scope_level TEXT, p_scope_key TEXT)
RETURNS TEXT
LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE WHEN p_scope_level = 'org' THEN 'org'
                ELSE p_scope_level || ':' || p_scope_key END
$$;

CREATE OR REPLACE FUNCTION brain_scope_widens(p_scope_level TEXT)
RETURNS BOOLEAN
LANGUAGE sql IMMUTABLE AS $$
    SELECT p_scope_level <> 'user'
$$;

CREATE OR REPLACE FUNCTION brain_fact_scope_keys(
    p_scope_level TEXT, p_scope_key TEXT, p_topics TEXT[]
) RETURNS SETOF TEXT
LANGUAGE sql STABLE AS $$
    SELECT brain_own_scope_key(p_scope_level, p_scope_key)
    UNION
    SELECT CASE WHEN a = 'everyone' THEN 'org' ELSE a END
    FROM brain_topics t,
         jsonb_array_elements_text(COALESCE(t.body -> 'audience', '[]'::jsonb)) a
    WHERE brain_scope_widens(p_scope_level) AND t.id = ANY (p_topics)
$$;

CREATE OR REPLACE VIEW v_brain_working_set AS
SELECT w.scope_key, w.fact_id, w.importance_base
FROM brain_working_set w
JOIN brain_facts f ON f.id = w.fact_id
WHERE brain_scope_widens(f.scope_level)
   OR w.scope_key = brain_own_scope_key(f.scope_level, f.scope_key);

INSERT INTO projection_backlog (projection, key)
SELECT 'brain_fact_index', id FROM brain_facts
WHERE scope_level = 'user'
ON CONFLICT DO NOTHING;
