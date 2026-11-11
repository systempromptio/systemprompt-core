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

-- The user-scope rule, in one place for SQL: `brain_scope_widens` is the
-- SQL half of `recall::visible::topic_widens`, and `brain_own_scope_key` the
-- key a fact is reachable through by its own scope. The working-set writers
-- (`brain_fact_scope_keys`) and the read-side admission view
-- (`v_brain_working_set`) both call them.

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
