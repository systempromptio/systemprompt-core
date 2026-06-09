-- Record the system-prompt override applied to a gateway request, if any.
--
-- The gateway can replace or strip the inbound system prompt before forwarding
-- upstream, driven by a profile `system_prompt_overrides` rule or a registered
-- extension override. This column captures which decision was applied (e.g.
-- `config:replace`, `extension:tenant-prompt:strip`) so an audit shows it. NULL
-- when no override matched and for non-gateway requests.

ALTER TABLE ai_requests
    ADD COLUMN IF NOT EXISTS system_prompt_override TEXT;
