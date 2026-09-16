-- Which client produced each request and on which inbound wire protocol.
-- Both columns are closed enums mirrored by systemprompt_models::wire::origin
-- (ClientKind / InboundWireProtocol); the CHECK lists must match their as_str()
-- values exactly. DEFAULT 'unknown' keeps a pre-attribution binary inserting
-- during the deploy window; the new binary always writes an explicit value.
ALTER TABLE ai_requests ADD COLUMN IF NOT EXISTS client_kind TEXT NOT NULL DEFAULT 'unknown';
ALTER TABLE ai_requests ADD COLUMN IF NOT EXISTS wire_protocol TEXT NOT NULL DEFAULT 'unknown';
ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_client_kind_check;
ALTER TABLE ai_requests ADD CONSTRAINT ai_requests_client_kind_check CHECK (client_kind IN (
    'claude-code', 'claude-desktop', 'codex', 'opencode', 'hermes', 'other', 'internal', 'unknown'));
ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_wire_protocol_check;
ALTER TABLE ai_requests ADD CONSTRAINT ai_requests_wire_protocol_check CHECK (wire_protocol IN (
    'anthropic.messages', 'openai.chat', 'openai.responses', 'internal', 'unknown'));

-- Backfill 1: server-internal requests. The gateway always sets a
-- gateway_conversation_id on an admitted request and always writes a payload
-- row; the internal AiService and image paths never do either.
UPDATE ai_requests r
   SET client_kind = 'internal', wire_protocol = 'internal'
 WHERE r.client_kind = 'unknown'
   AND r.gateway_conversation_id IS NULL
   AND r.client_session_id IS NULL
   AND r.status <> 'rejected'
   AND NOT EXISTS (SELECT 1 FROM ai_request_payloads p WHERE p.ai_request_id = r.id);

-- Backfill 2: gateway requests whose JSON body was retained. The Claude
-- metadata.user_id shape cannot tell Claude Desktop from Claude Code; both are
-- labelled claude-code, the only one that reached this gateway historically.
-- Rows with no retained evidence stay 'unknown' rather than being guessed.
UPDATE ai_requests r
   SET client_kind = d.client_kind,
       wire_protocol = CASE
           WHEN d.client_kind = 'claude-code' THEN 'anthropic.messages'
           WHEN d.client_kind = 'codex'       THEN 'openai.responses'
           WHEN d.client_kind = 'opencode'    THEN 'openai.chat'
           WHEN d.body ? 'input'              THEN 'openai.responses'
           WHEN d.body ? 'messages' AND (d.body ? 'system'
                 OR (jsonb_typeof(d.body -> 'tools') = 'array' AND EXISTS (
                     SELECT 1 FROM jsonb_array_elements(d.body -> 'tools') t WHERE t ? 'input_schema')))
                                              THEN 'anthropic.messages'
           WHEN d.body ? 'messages' AND (d.body ? 'max_completion_tokens'
                 OR (jsonb_typeof(d.body -> 'messages') = 'array' AND EXISTS (
                     SELECT 1 FROM jsonb_array_elements(d.body -> 'messages') m
                     WHERE m ->> 'role' IN ('system', 'developer', 'tool'))))
                                              THEN 'openai.chat'
           ELSE 'unknown' END
  FROM (
    SELECT p.ai_request_id, p.request_body AS body,
           CASE
             WHEN p.request_body #>> '{client_metadata,x-codex-turn-metadata}' IS NOT NULL THEN 'codex'
             WHEN p.request_body #>> '{metadata,user_id}' LIKE 'user\_%\_account\_%\_session\_%' THEN 'claude-code'
             WHEN p.request_body #>> '{metadata,user_id}' LIKE '{%' THEN 'opencode'
             ELSE 'other' END AS client_kind
      FROM ai_request_payloads p
     WHERE jsonb_typeof(p.request_body) = 'object'
  ) d
 WHERE d.ai_request_id = r.id AND r.client_kind = 'unknown';
