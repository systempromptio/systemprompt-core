-- Record the provider that actually served a gateway request.
--
-- `provider` is the route's primary upstream, fixed when the row is opened.
-- A route with a `fallback_provider` may finish the request elsewhere after
-- the primary exhausts its retry budget or is unreachable, and the row must
-- say so: the cost is the served provider's rate, and an operator reading a
-- failover incident needs the row to name where the tokens went. NULL means
-- the primary served, and for every row written before this migration.
ALTER TABLE ai_requests
    ADD COLUMN IF NOT EXISTS served_provider TEXT;
