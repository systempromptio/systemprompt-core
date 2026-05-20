-- RFC 8693 act-chain attribution for governance decisions.
--
-- `act_chain` records the delegation lineage of the token that authorised
-- this decision: a JSON array of `Actor` values in outermost-first order.
-- An empty array (the default) means the request used a direct,
-- non-delegated token — equivalent to no `act` claim being present.

ALTER TABLE governance_decisions
    ADD COLUMN IF NOT EXISTS act_chain JSONB NOT NULL DEFAULT '[]'::jsonb;

CREATE INDEX IF NOT EXISTS idx_governance_decisions_act_chain
    ON governance_decisions USING GIN (act_chain);
