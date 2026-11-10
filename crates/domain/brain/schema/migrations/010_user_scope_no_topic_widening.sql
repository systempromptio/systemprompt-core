-- A user-scoped fact is reachable only through its owner's key: topic
-- audiences widen project, group and org facts, never a user's. The key
-- derivation is one function the per-fact refresh, the rebuild and the
-- consistency count share. The DELETE removes exactly the working-set rows
-- of user-scoped facts whose key is not `user:<owner>` (the rows a topic
-- audience added); the owner's own row is kept.
-- @cost: rows=0 measured=0.0s triggers=live

CREATE OR REPLACE FUNCTION brain_fact_scope_keys(
    p_scope_level TEXT, p_scope_key TEXT, p_topics TEXT[]
) RETURNS SETOF TEXT
LANGUAGE sql STABLE AS $$
    SELECT CASE WHEN p_scope_level = 'org' THEN 'org'
                ELSE p_scope_level || ':' || p_scope_key END
    UNION
    SELECT CASE WHEN a = 'everyone' THEN 'org' ELSE a END
    FROM brain_topics t,
         jsonb_array_elements_text(COALESCE(t.body -> 'audience', '[]'::jsonb)) a
    WHERE p_scope_level <> 'user' AND t.id = ANY (p_topics)
$$;

DELETE FROM brain_working_set w
USING brain_facts f
WHERE w.fact_id = f.id
  AND f.scope_level = 'user'
  AND w.scope_key <> 'user:' || f.scope_key;
