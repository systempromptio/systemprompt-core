-- Existing installations used content-deduplicated snapshots and a global
-- resource key. Source polling must retain every observation, while resource
-- names are tenant scoped.
ALTER TABLE managed_source_snapshots DROP CONSTRAINT IF EXISTS managed_source_snapshots_owner_id_source_id_digest_key;
ALTER TABLE managed_resources DROP CONSTRAINT IF EXISTS managed_resources_kind_resource_key_key;
CREATE UNIQUE INDEX IF NOT EXISTS managed_source_snapshots_owner_id_id
    ON managed_source_snapshots(owner_id,id);
CREATE UNIQUE INDEX IF NOT EXISTS managed_resources_owner_kind_key
    ON managed_resources(owner_id,kind,resource_key);

CREATE TABLE IF NOT EXISTS managed_reconciliations (
    id TEXT PRIMARY KEY, owner_id TEXT NOT NULL, resource_id TEXT NOT NULL,
    upstream_base_revision_id TEXT NOT NULL, managed_candidate_revision_id TEXT NOT NULL,
    incoming_revision_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','resolved','withdrawal_proposed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(), resolved_revision_id TEXT,
    resolved_by TEXT REFERENCES users(id), resolved_at TIMESTAMPTZ,
    FOREIGN KEY(owner_id,resource_id) REFERENCES managed_resources(owner_id,id),
    FOREIGN KEY(owner_id,resource_id,upstream_base_revision_id) REFERENCES managed_revisions(owner_id,resource_id,id),
    FOREIGN KEY(owner_id,resource_id,managed_candidate_revision_id) REFERENCES managed_revisions(owner_id,resource_id,id),
    FOREIGN KEY(owner_id,resource_id,incoming_revision_id) REFERENCES managed_revisions(owner_id,resource_id,id),
    FOREIGN KEY(owner_id,resource_id,resolved_revision_id) REFERENCES managed_revisions(owner_id,resource_id,id),
    UNIQUE(owner_id,resource_id,managed_candidate_revision_id,incoming_revision_id)
);
CREATE TABLE IF NOT EXISTS managed_reconciliation_conflicts (
    reconciliation_id TEXT NOT NULL REFERENCES managed_reconciliations(id), path TEXT NOT NULL,
    base_digest TEXT, candidate_digest TEXT, incoming_digest TEXT,
    resolution TEXT CHECK (resolution IN ('candidate','incoming','manual','delete')),
    resolved_digest TEXT, PRIMARY KEY(reconciliation_id,path)
);
CREATE TABLE IF NOT EXISTS managed_withdrawal_proposals (
    id TEXT PRIMARY KEY, owner_id TEXT NOT NULL, resource_id TEXT NOT NULL, snapshot_id TEXT NOT NULL,
    reason TEXT NOT NULL CHECK (length(reason) BETWEEN 1 AND 4000),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(), decided_by TEXT REFERENCES users(id), decided_at TIMESTAMPTZ,
    FOREIGN KEY(owner_id,resource_id) REFERENCES managed_resources(owner_id,id),
    FOREIGN KEY(owner_id,snapshot_id) REFERENCES managed_source_snapshots(owner_id,id),
    UNIQUE(owner_id,resource_id,snapshot_id)
);
CREATE TABLE IF NOT EXISTS managed_distribution_deliveries (
    id TEXT PRIMARY KEY, owner_id TEXT NOT NULL,
    outbox_id TEXT NOT NULL REFERENCES managed_distribution_outbox(id), publication_id TEXT NOT NULL,
    generation BIGINT NOT NULL CHECK (generation > 0), bundle_digest TEXT,
    status TEXT NOT NULL CHECK (status IN ('claimed','distributed','failed')), claim_token TEXT NOT NULL,
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT now(), delivered_at TIMESTAMPTZ, error TEXT,
    FOREIGN KEY(owner_id,publication_id) REFERENCES managed_publications(owner_id,id),
    UNIQUE(outbox_id), UNIQUE(owner_id,claim_token)
);
CREATE TABLE IF NOT EXISTS managed_installation_receipts (
    id TEXT PRIMARY KEY, owner_id TEXT NOT NULL, installation_id TEXT NOT NULL,
    publication_id TEXT NOT NULL, resource_id TEXT NOT NULL, generation BIGINT NOT NULL CHECK (generation > 0),
    bundle_digest TEXT NOT NULL CHECK (bundle_digest ~ '^[0-9a-f]{64}$'), installed_manifest JSONB NOT NULL,
    client_evidence JSONB NOT NULL, verified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id,resource_id,publication_id) REFERENCES managed_publications(owner_id,resource_id,id),
    UNIQUE(owner_id,installation_id,resource_id,generation), UNIQUE(owner_id,id)
);
CREATE TABLE IF NOT EXISTS managed_invocation_attributions (
    id TEXT PRIMARY KEY, owner_id TEXT NOT NULL, invocation_id TEXT NOT NULL,
    installation_id TEXT, resource_id TEXT, revision_id TEXT, publication_generation BIGINT,
    traffic_class TEXT NOT NULL CHECK (traffic_class IN ('production','fixture','live_evaluation','suggestion','judge')),
    status TEXT NOT NULL CHECK (status IN ('verified','revision_unknown','unsupported','historical')),
    receipt_id TEXT, authenticated_evidence JSONB NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id,receipt_id) REFERENCES managed_installation_receipts(owner_id,id),
    UNIQUE(owner_id,invocation_id)
);
DROP TRIGGER IF EXISTS managed_installation_receipts_immutable ON managed_installation_receipts;
CREATE TRIGGER managed_installation_receipts_immutable BEFORE UPDATE ON managed_installation_receipts FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_invocation_attributions_immutable ON managed_invocation_attributions;
CREATE TRIGGER managed_invocation_attributions_immutable BEFORE UPDATE ON managed_invocation_attributions FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
