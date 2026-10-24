-- The `governance` config plane: the policy chain `services/governance/config.yaml`
-- declares, one row per entity. `governance` holds the chain's own settings
-- (enabled, default mode); `policy/<id>` holds one policy entry with its
-- position in the chain. Every node builds its governance engine from these
-- rows and rebuilds it when the `config:governance` notification arrives;
-- `config apply` and `/admin/sync` are the writers.
CREATE TABLE IF NOT EXISTS governance_config (
    entity TEXT PRIMARY KEY CHECK (entity = 'governance' OR entity ~ '^policy/.+$'),
    value TEXT NOT NULL,
    source TEXT NOT NULL
        CONSTRAINT governance_config_source_check
        CHECK (source ~ '^(code:.+|bundle:.+|console:.+|migration)$'),
    changed_by TEXT,
    changed_at TIMESTAMPTZ,
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
