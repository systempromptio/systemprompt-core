-- Record the client-requested model alongside the served model on ai_requests.
--
-- The gateway rewrites a requested model through its routes and the upstream
-- provider may substitute its own (e.g. a request for `gpt-5` served by MiniMax
-- as `MiniMax-M3`). `model` holds the served model; `requested_model` preserves
-- what the client actually asked for so an audit shows both. NULL for rows
-- written before this migration and for non-gateway requests.

ALTER TABLE ai_requests
    ADD COLUMN IF NOT EXISTS requested_model TEXT;
