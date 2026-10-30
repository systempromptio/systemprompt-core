-- Per fact kind: facts by status, how often recall injected them, how often
-- a response cited or a person corrected them, open disputes and facts past
-- their review date. The rates divide by injections, so a kind never
-- injected reports a NULL rate rather than zero.
--
-- Replaced in place: CREATE OR REPLACE can append a column but cannot drop,
-- rename or reorder one; a change that does ships a migration that drops the
-- view first.

CREATE OR REPLACE VIEW v_brain_kind_health AS
WITH facts AS (
    SELECT
        f.kind,
        COUNT(*) AS facts,
        COUNT(*) FILTER (WHERE f.status = 'candidate') AS candidate,
        COUNT(*) FILTER (WHERE f.status = 'proposed') AS proposed,
        COUNT(*) FILTER (WHERE f.status = 'confirmed') AS confirmed,
        COUNT(*) FILTER (WHERE f.status = 'disputed') AS disputed,
        COUNT(*) FILTER (WHERE f.status = 'stale') AS stale,
        COUNT(*) FILTER (WHERE f.status = 'superseded') AS superseded,
        COUNT(*) FILTER (WHERE f.status = 'retired') AS retired,
        COUNT(*) FILTER (
            WHERE f.status = 'confirmed' AND f.review_after < now()
        ) AS review_due
    FROM brain_facts f
    GROUP BY f.kind
),
injections AS (
    SELECT f.kind, COUNT(*) AS injected
    FROM brain_recalls r
    CROSS JOIN LATERAL jsonb_array_elements(r.injected) AS i(fact)
    JOIN brain_facts f ON f.id = i.fact->>'id'
    WHERE r.skipped IS NULL
    GROUP BY f.kind
),
outcomes AS (
    SELECT
        f.kind,
        COUNT(*) FILTER (WHERE o.signal = 'cited_in_response') AS cited,
        COUNT(*) FILTER (WHERE o.signal = 'user_corrected_fact') AS corrected
    FROM brain_outcomes o
    JOIN brain_facts f ON f.id = o.fact_id
    GROUP BY f.kind
),
disputes AS (
    SELECT f.kind, COUNT(*) AS open_disputes
    FROM brain_disputes d
    JOIN brain_facts f ON f.id = d.fact_id
    WHERE d.status = 'open'
    GROUP BY f.kind
)
SELECT
    facts.kind,
    facts.facts,
    facts.candidate,
    facts.proposed,
    facts.confirmed,
    facts.disputed,
    facts.stale,
    facts.superseded,
    facts.retired,
    facts.review_due,
    COALESCE(injections.injected, 0) AS injected,
    COALESCE(outcomes.cited, 0) AS cited,
    COALESCE(outcomes.corrected, 0) AS corrected,
    outcomes.cited::float8 / NULLIF(injections.injected, 0) AS cited_rate,
    outcomes.corrected::float8 / NULLIF(injections.injected, 0) AS corrected_rate,
    COALESCE(disputes.open_disputes, 0) AS open_disputes
FROM facts
LEFT JOIN injections ON injections.kind = facts.kind
LEFT JOIN outcomes ON outcomes.kind = facts.kind
LEFT JOIN disputes ON disputes.kind = facts.kind;
