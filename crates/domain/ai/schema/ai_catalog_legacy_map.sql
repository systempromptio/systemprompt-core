-- How each pre-0.73 provider entry name maps onto the 0.73 catalog: the
-- provider, endpoint and credential `systemprompt admin config migrate`
-- folded it into. `config migrate --data` writes it from the install's
-- legacy_map.yaml and moves stored personal keys and the audit's
-- credential/endpoint columns through it; ai_requests.provider keeps its
-- historical values so reports stay comparable.
CREATE TABLE IF NOT EXISTS ai_catalog_legacy_map (
    legacy_provider TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    credential TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
