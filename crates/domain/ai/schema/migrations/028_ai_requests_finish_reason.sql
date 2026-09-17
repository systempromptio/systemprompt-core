-- Persist the upstream's own finish reason beside the normalised status.
--
-- `status` says whether the turn settled; the provider's finish reason says
-- how it ended — Gemini `STOP`, `MAX_TOKENS`, `SAFETY`, `MALFORMED_FUNCTION_CALL`,
-- OpenAI `content_filter`, Anthropic `refusal`. The canonical stop reason
-- folds most of those into `end_turn` on the wire, so without this column a
-- refused or malformed turn was indistinguishable from a clean one in the
-- audit row. NULL for every row written before this migration and for a
-- request that never reached a terminal event.
ALTER TABLE ai_requests
    ADD COLUMN IF NOT EXISTS finish_reason TEXT;
