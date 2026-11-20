-- documents.agent_content: whether an agent has ever written any of the
-- document's content. It is set in the transaction that stores an agent's
-- revision (a proposal included), lands an agent's proposal, creates a
-- document as an agent or creates the copy of a flagged document, and it is
-- never cleared. The rendered view of a flagged HTML document is served under
-- renderedHtml.agentAuthored.
--
-- No backfill: a migration past the files floor is DDL only, because a fresh
-- install stamps it without running it. A document written before this
-- column existed is read as agent content from the same evidence a backfill
-- would have used (created_by_agent, an agent revision or an agent
-- proposal); see DocumentRepository::agent_content.

ALTER TABLE documents
    ADD COLUMN IF NOT EXISTS agent_content BOOLEAN NOT NULL DEFAULT false;
