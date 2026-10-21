-- Record the OAuth client the decision was made for.
--
-- The client id comes from the validated bearer token and is the only
-- verified fact that distinguishes one ingress (the bridge, a first-party
-- app, a token-exchange delegate) from another. Nullable: API keys and
-- legacy tokens carry none, and NULL is the truthful value for them.

ALTER TABLE governance_decisions ADD COLUMN IF NOT EXISTS client_id TEXT;

CREATE INDEX IF NOT EXISTS idx_governance_decisions_client ON governance_decisions(client_id);
