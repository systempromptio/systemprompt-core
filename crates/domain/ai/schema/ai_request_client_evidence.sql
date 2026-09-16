-- What the wire carried about the client of one request, kept beside the
-- classification on ai_requests so it can be audited or re-derived. The
-- contract columns on ai_requests are NOT NULL; these record facts that may
-- legitimately be absent, so NULL means "not presented". Every enum mirrors
-- systemprompt_models::wire::origin and every length bound matches the
-- truncation applied when ClientEvidence is built.
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
