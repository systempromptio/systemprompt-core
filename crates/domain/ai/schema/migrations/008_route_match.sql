-- Record how a gateway request's route was selected, if non-trivially.
--
-- Beyond the model glob, a route may carry request-shape predicates (a `when`
-- block) and an extension may re-route the request programmatically. This
-- column captures which predicates matched and/or which selector fired (e.g.
-- `when:thinking,min_reasoning_effort`, `selector:token-budget`) so an audit
-- shows why a request landed on its backend. NULL for a plain model-only match
-- and for non-gateway requests.

ALTER TABLE ai_requests
    ADD COLUMN IF NOT EXISTS route_match TEXT;
