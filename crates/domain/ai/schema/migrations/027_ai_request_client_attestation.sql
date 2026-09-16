-- How strongly each request's client is evidenced, and the evidence itself.
-- client_attestation is a closed enum mirrored by
-- systemprompt_models::wire::origin::ClientAttestation; client_kind gains
-- 'pi'. DEFAULT 'unknown' keeps a pre-attestation binary inserting during the
-- deploy window; the new binary always writes an explicit value.
ALTER TABLE ai_requests ADD COLUMN IF NOT EXISTS client_attestation TEXT NOT NULL DEFAULT 'unknown';
ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_client_attestation_check;
ALTER TABLE ai_requests ADD CONSTRAINT ai_requests_client_attestation_check CHECK (client_attestation IN (
    'host-token', 'bridge-secret', 'declared', 'native-marker', 'user-agent', 'none', 'internal', 'unknown'));
ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_client_kind_check;
ALTER TABLE ai_requests ADD CONSTRAINT ai_requests_client_kind_check CHECK (client_kind IN (
    'claude-code', 'claude-desktop', 'codex', 'opencode', 'hermes', 'pi', 'other', 'internal', 'unknown'));

-- Backfill 1: server-internal rows pair with the internal tier.
UPDATE ai_requests
   SET client_attestation = 'internal'
 WHERE client_attestation = 'unknown' AND client_kind = 'internal';

-- Backfill 2: rows migration 026 classified from a retained body were named
-- by a native marker; nothing else about the wire survives, so the rest stay
-- 'unknown' rather than being labelled with a tier they never earned.
UPDATE ai_requests r
   SET client_attestation = 'native-marker'
 WHERE r.client_attestation = 'unknown'
   AND r.client_kind IN ('claude-code', 'codex', 'opencode')
   AND EXISTS (SELECT 1 FROM ai_request_payloads p
                WHERE p.ai_request_id = r.id AND jsonb_typeof(p.request_body) = 'object');

CREATE TABLE IF NOT EXISTS ai_request_client_evidence (
    ai_request_id VARCHAR(255) PRIMARY KEY REFERENCES ai_requests(id) ON DELETE CASCADE,
    kind_source TEXT NOT NULL
        CONSTRAINT ai_request_client_evidence_kind_source_check CHECK (kind_source IN (
            'host-token', 'bridge-secret', 'declared', 'native-marker', 'user-agent', 'none', 'internal', 'unknown')),
    attested_host TEXT
        CONSTRAINT ai_request_client_evidence_attested_host_check CHECK (attested_host IS NULL OR attested_host IN (
            'claude-code', 'claude-desktop', 'codex', 'opencode', 'hermes', 'pi', 'other', 'internal', 'unknown')),
    declared_client TEXT CHECK (length(declared_client) <= 64),
    native_marker TEXT
        CONSTRAINT ai_request_client_evidence_native_marker_check CHECK (native_marker IS NULL OR native_marker IN (
            'claude-metadata-user-id', 'codex-turn-metadata', 'opencode-session-json')),
    ua_product TEXT CHECK (length(ua_product) <= 64),
    ua_version TEXT CHECK (length(ua_version) <= 64),
    sdk_lang TEXT CHECK (length(sdk_lang) <= 64),
    sdk_package_version TEXT CHECK (length(sdk_package_version) <= 64),
    sdk_runtime TEXT CHECK (length(sdk_runtime) <= 64),
    sdk_runtime_version TEXT CHECK (length(sdk_runtime_version) <= 64),
    sdk_os TEXT CHECK (length(sdk_os) <= 64),
    sdk_arch TEXT CHECK (length(sdk_arch) <= 64),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
