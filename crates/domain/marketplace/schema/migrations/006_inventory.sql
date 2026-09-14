CREATE TABLE IF NOT EXISTS managed_inventory_state (
    owner_id TEXT PRIMARY KEY REFERENCES users(id),
    generation BIGINT NOT NULL DEFAULT 0,
    observed_at TIMESTAMPTZ,
    entries BIGINT NOT NULL DEFAULT 0,
    last_error TEXT
);
CREATE TABLE IF NOT EXISTS managed_inventory_entries (
    owner_id TEXT NOT NULL REFERENCES users(id),
    entry_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    resource_key TEXT NOT NULL,
    origin TEXT NOT NULL,
    configured_key TEXT,
    resource_id TEXT,
    source_id TEXT,
    availability TEXT NOT NULL,
    latest_revision_id TEXT,
    published_revision_id TEXT,
    diagnostic TEXT,
    first_observed_at TIMESTAMPTZ NOT NULL,
    last_observed_at TIMESTAMPTZ NOT NULL,
    generation BIGINT NOT NULL,
    PRIMARY KEY(owner_id,entry_id),
    FOREIGN KEY(owner_id,resource_id) REFERENCES managed_resources(owner_id,id),
    FOREIGN KEY(owner_id,source_id) REFERENCES managed_sources(owner_id,id)
);
CREATE INDEX IF NOT EXISTS managed_inventory_list ON managed_inventory_entries(owner_id,entry_id);
CREATE TABLE IF NOT EXISTS managed_inventory_membership (
    owner_id TEXT NOT NULL,
    entry_id TEXT NOT NULL,
    effective_from TIMESTAMPTZ NOT NULL,
    effective_until TIMESTAMPTZ,
    record JSONB NOT NULL,
    PRIMARY KEY(owner_id,entry_id,effective_from),
    FOREIGN KEY(owner_id,entry_id) REFERENCES managed_inventory_entries(owner_id,entry_id),
    CHECK(effective_until IS NULL OR effective_until>=effective_from)
);
CREATE UNIQUE INDEX IF NOT EXISTS managed_inventory_open_membership ON managed_inventory_membership(owner_id,entry_id) WHERE effective_until IS NULL;
CREATE TABLE IF NOT EXISTS managed_inventory_bindings (
    owner_id TEXT NOT NULL,
    entry_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    bound_by TEXT NOT NULL REFERENCES users(id),
    bound_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(owner_id,entry_id),
    UNIQUE(owner_id,resource_id),
    FOREIGN KEY(owner_id,entry_id) REFERENCES managed_inventory_entries(owner_id,entry_id),
    FOREIGN KEY(owner_id,resource_id) REFERENCES managed_resources(owner_id,id)
);
CREATE TABLE IF NOT EXISTS managed_inventory_captures (
    owner_id TEXT NOT NULL,
    entry_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    status TEXT NOT NULL,
    revision_id TEXT,
    reconciliation_id TEXT,
    diagnostic TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(owner_id,entry_id,operation_id),
    FOREIGN KEY(owner_id,entry_id) REFERENCES managed_inventory_entries(owner_id,entry_id),
    FOREIGN KEY(owner_id,revision_id) REFERENCES managed_revisions(owner_id,id)
);
CREATE TABLE IF NOT EXISTS managed_inventory_authoring_heads (
    owner_id TEXT NOT NULL,
    entry_id TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    PRIMARY KEY(owner_id,entry_id),
    FOREIGN KEY(owner_id,entry_id) REFERENCES managed_inventory_entries(owner_id,entry_id),
    FOREIGN KEY(owner_id,revision_id) REFERENCES managed_revisions(owner_id,id)
);
DROP TRIGGER IF EXISTS managed_inventory_bindings_immutable ON managed_inventory_bindings;
CREATE TRIGGER managed_inventory_bindings_immutable BEFORE UPDATE ON managed_inventory_bindings FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
CREATE TABLE IF NOT EXISTS managed_inventory_observations (
    owner_id TEXT NOT NULL REFERENCES users(id),
    generation BIGINT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    entries BIGINT NOT NULL,
    PRIMARY KEY(owner_id,generation)
);
CREATE INDEX IF NOT EXISTS managed_inventory_observations_time ON managed_inventory_observations(owner_id,observed_at);
