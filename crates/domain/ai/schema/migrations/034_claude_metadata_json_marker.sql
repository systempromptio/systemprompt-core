-- @cost: rows=0 measured=20ms triggers=live
--
-- Claude Code ≥ 2.1.25x stamps `metadata.user_id` as a JSON string
-- (`{"account_uuid","device_id","session_id"}`). Migration 026 and the
-- runtime classifier read any JSON there as OpenCode's session marker, so
-- every modern Claude Code and Claude Desktop (Cowork) request that did not
-- carry a bridge host token was recorded as `opencode` on `openai.chat` —
-- 3,644 of 3,644 marker-bearing rows on one production instance, none of
-- them OpenCode. The marker vocabulary is re-cut around what Claude Code
-- actually sends: its billing entrypoint (`cc_entrypoint=` in the first
-- system block, the one signal that tells Desktop from the CLI) and its two
-- metadata grammars.
--
-- This migration corrects the vocabulary. Re-deriving the mislabelled rows
-- themselves is left to the installation that owns the per-row triggers on
-- `ai_requests`, because a blind UPDATE here fans out through them (113 ms
-- per row on the instance above); see the astound `102` migration.
ALTER TABLE ai_request_client_evidence DROP CONSTRAINT IF EXISTS ai_request_client_evidence_native_marker_check;
UPDATE ai_request_client_evidence
   SET native_marker = 'claude-metadata-json'
 WHERE native_marker = 'opencode-session-json';
ALTER TABLE ai_request_client_evidence ADD CONSTRAINT ai_request_client_evidence_native_marker_check
    CHECK (native_marker IS NULL OR native_marker IN (
        'claude-desktop-entrypoint', 'claude-cli-entrypoint', 'claude-metadata-user-id',
        'claude-metadata-json', 'codex-turn-metadata'));
