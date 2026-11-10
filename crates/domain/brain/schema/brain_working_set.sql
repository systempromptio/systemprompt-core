CREATE TABLE IF NOT EXISTS brain_working_set (
    scope_key TEXT NOT NULL,
    fact_id TEXT NOT NULL REFERENCES brain_facts(id) ON DELETE CASCADE,
    importance_base FLOAT8 NOT NULL,
    PRIMARY KEY (scope_key, fact_id)
);

CREATE INDEX IF NOT EXISTS brain_working_set_scope_idx
    ON brain_working_set (scope_key, importance_base DESC, fact_id);
CREATE INDEX IF NOT EXISTS brain_working_set_fact_idx
    ON brain_working_set (fact_id, scope_key);

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
