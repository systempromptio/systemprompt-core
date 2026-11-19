-- documents.agent_content: whether an agent has ever written any of the
-- document's content. It is set in the transaction that stores an agent's
-- revision (a proposal included), lands an agent's proposal, creates a
-- document as an agent or creates the copy of a flagged document, and it is
-- never cleared. The rendered view of
-- a flagged HTML document is served under renderedHtml.agentAuthored.
--
-- The backfill errs towards flagging: a document an agent created, one with
-- any revision or proposal an agent stored, and, transitively, a copy of any
-- of these (a copy's first revision says "copy of <source id>").

ALTER TABLE documents
    ADD COLUMN IF NOT EXISTS agent_content BOOLEAN NOT NULL DEFAULT false;

WITH RECURSIVE copies AS MATERIALIZED (
    SELECT d.id AS copy_id, substr(r.message, 9) AS source_id
    FROM documents d
    JOIN document_revisions r
      ON r.document_id = d.id
     AND r.parent_revision_id IS NULL
     AND r.message LIKE 'copy of %'
),
flagged (id) AS (
    SELECT d.id
    FROM documents d
    WHERE d.created_by_agent IS NOT NULL
       OR EXISTS (
           SELECT 1 FROM document_revisions r
           WHERE r.document_id = d.id AND r.actor_kind = 'agent'
       )
       OR EXISTS (
           SELECT 1 FROM document_proposals p
           WHERE p.document_id = d.id AND p.actor_kind = 'agent'
       )
    UNION
    SELECT c.copy_id
    FROM copies c
    JOIN flagged f ON c.source_id = f.id::text
)
UPDATE documents d
SET agent_content = true
FROM flagged f
WHERE d.id = f.id AND NOT d.agent_content;
