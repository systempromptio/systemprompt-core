-- Immutable content and explicit publication: imports never move active revisions.
CREATE TABLE IF NOT EXISTS managed_sources (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 200),
    kind TEXT NOT NULL CHECK (kind IN ('git', 'local_tree', 'managed')),
    specification JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(owner_id, name),
    UNIQUE(owner_id, id)
);
CREATE TABLE IF NOT EXISTS managed_source_snapshots (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    digest TEXT NOT NULL CHECK (digest ~ '^[0-9a-f]{64}$'),
    provenance JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id, source_id) REFERENCES managed_sources(owner_id, id),
    UNIQUE(owner_id, source_id, id),
    UNIQUE(owner_id, id)
);
CREATE TABLE IF NOT EXISTS managed_assets (
    owner_id TEXT NOT NULL REFERENCES users(id),
    digest TEXT NOT NULL CHECK (digest ~ '^[0-9a-f]{64}$'),
    content BYTEA NOT NULL CHECK (octet_length(content) <= 16777216),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(owner_id, digest)
);
CREATE TABLE IF NOT EXISTS managed_resources (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    upstream_key TEXT NOT NULL CHECK (length(upstream_key) BETWEEN 1 AND 200),
    kind TEXT NOT NULL CHECK (kind IN ('skill','plugin','marketplace','supporting')),
    resource_key TEXT NOT NULL CHECK (length(resource_key) BETWEEN 1 AND 200),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id, source_id) REFERENCES managed_sources(owner_id, id),
    UNIQUE(source_id, upstream_key),
    UNIQUE(owner_id, kind, resource_key),
    UNIQUE(owner_id, id),
    UNIQUE(owner_id, source_id, id)
);
CREATE TABLE IF NOT EXISTS managed_revisions (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    snapshot_id TEXT NOT NULL,
    parent_id TEXT,
    digest TEXT NOT NULL CHECK (digest ~ '^[0-9a-f]{64}$'),
    manifest JSONB NOT NULL,
    rationale TEXT NOT NULL CHECK (length(rationale) BETWEEN 1 AND 4000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id, source_id, resource_id) REFERENCES managed_resources(owner_id, source_id, id),
    FOREIGN KEY(owner_id, source_id, snapshot_id) REFERENCES managed_source_snapshots(owner_id, source_id, id),
    UNIQUE(owner_id, resource_id, id),
    UNIQUE(owner_id, id),
    FOREIGN KEY(owner_id, resource_id, parent_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    UNIQUE(resource_id, digest)
);
CREATE TABLE IF NOT EXISTS managed_revision_assets (
    owner_id TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    path TEXT NOT NULL,
    digest TEXT NOT NULL,
    PRIMARY KEY(revision_id, path),
    FOREIGN KEY(owner_id, revision_id) REFERENCES managed_revisions(owner_id, id),
    FOREIGN KEY(owner_id, digest) REFERENCES managed_assets(owner_id, digest)
);
CREATE TABLE IF NOT EXISTS managed_revision_dependencies (
    owner_id TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    dependency_id TEXT NOT NULL,
    PRIMARY KEY(revision_id, dependency_id),
    FOREIGN KEY(owner_id, revision_id) REFERENCES managed_revisions(owner_id, id),
    FOREIGN KEY(owner_id, dependency_id) REFERENCES managed_revisions(owner_id, id),
    CHECK (revision_id <> dependency_id)
);
-- No UPDATE path exists for immutable provenance or content. Deletion remains
-- available to retention after all references have been removed.
CREATE OR REPLACE FUNCTION reject_managed_content_update() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'managed content is immutable' USING ERRCODE = '23514';
END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS managed_sources_immutable ON managed_sources;
CREATE TRIGGER managed_sources_immutable BEFORE UPDATE ON managed_sources FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_snapshots_immutable ON managed_source_snapshots;
CREATE TRIGGER managed_snapshots_immutable BEFORE UPDATE ON managed_source_snapshots FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_assets_immutable ON managed_assets;
CREATE TRIGGER managed_assets_immutable BEFORE UPDATE ON managed_assets FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_resources_immutable ON managed_resources;
CREATE TRIGGER managed_resources_immutable BEFORE UPDATE ON managed_resources FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_revisions_immutable ON managed_revisions;
CREATE TRIGGER managed_revisions_immutable BEFORE UPDATE ON managed_revisions FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_revision_assets_immutable ON managed_revision_assets;
CREATE TRIGGER managed_revision_assets_immutable BEFORE UPDATE ON managed_revision_assets FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_dependencies_immutable ON managed_revision_dependencies;
CREATE TRIGGER managed_dependencies_immutable BEFORE UPDATE ON managed_revision_dependencies FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();

CREATE TABLE IF NOT EXISTS managed_publication_reviews (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    resource_id TEXT NOT NULL,
    revision_id TEXT,
    action TEXT NOT NULL CHECK (action IN ('initial_adoption','publish_improvement','withdraw','rollback')),
    bundle_digest TEXT CHECK (bundle_digest IS NULL OR bundle_digest ~ '^[0-9a-f]{64}$'),
    comparison_evidence JSONB NOT NULL,
    limitations TEXT NOT NULL CHECK (length(limitations) <= 4000),
    reviewer_id TEXT NOT NULL REFERENCES users(id),
    expected_generation BIGINT NOT NULL CHECK (expected_generation >= 0),
    request_digest TEXT NOT NULL CHECK (request_digest ~ '^[0-9a-f]{64}$'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id, resource_id) REFERENCES managed_resources(owner_id, id),
    FOREIGN KEY(owner_id, resource_id, revision_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    UNIQUE(owner_id, id)
);
CREATE TABLE IF NOT EXISTS managed_publications (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    review_id TEXT NOT NULL,
    generation BIGINT NOT NULL CHECK (generation > 0),
    action TEXT NOT NULL CHECK (action IN ('initial_adoption','publish_improvement','withdraw','rollback')),
    revision_id TEXT,
    bundle_digest TEXT CHECK (bundle_digest IS NULL OR bundle_digest ~ '^[0-9a-f]{64}$'),
    operation_key TEXT NOT NULL CHECK (length(operation_key) BETWEEN 1 AND 200),
    request_digest TEXT NOT NULL CHECK (request_digest ~ '^[0-9a-f]{64}$'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id, resource_id) REFERENCES managed_resources(owner_id, id),
    FOREIGN KEY(owner_id, review_id) REFERENCES managed_publication_reviews(owner_id, id),
    FOREIGN KEY(owner_id, resource_id, revision_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    UNIQUE(owner_id, resource_id, generation),
    UNIQUE(owner_id, operation_key),
    UNIQUE(owner_id, id),
    UNIQUE(owner_id, resource_id, id)
);
CREATE TABLE IF NOT EXISTS managed_publication_selections (
    owner_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    generation BIGINT NOT NULL CHECK (generation > 0),
    state TEXT NOT NULL CHECK (state IN ('published','withdrawn')),
    publication_id TEXT NOT NULL,
    revision_id TEXT,
    bundle_digest TEXT CHECK (bundle_digest IS NULL OR bundle_digest ~ '^[0-9a-f]{64}$'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(owner_id, resource_id),
    FOREIGN KEY(owner_id, resource_id, publication_id) REFERENCES managed_publications(owner_id, resource_id, id),
    FOREIGN KEY(owner_id, resource_id, revision_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    CHECK ((state = 'published' AND revision_id IS NOT NULL AND bundle_digest IS NOT NULL)
        OR (state = 'withdrawn' AND revision_id IS NULL AND bundle_digest IS NULL))
);
CREATE TABLE IF NOT EXISTS managed_distribution_outbox (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    publication_id TEXT NOT NULL,
    generation BIGINT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ,
    FOREIGN KEY(owner_id, publication_id) REFERENCES managed_publications(owner_id, id),
    UNIQUE(publication_id)
);
CREATE TABLE IF NOT EXISTS managed_reconciliations (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    upstream_base_revision_id TEXT NOT NULL,
    managed_candidate_revision_id TEXT NOT NULL,
    incoming_revision_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','resolved','withdrawal_proposed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_revision_id TEXT,
    resolved_by TEXT REFERENCES users(id),
    resolved_at TIMESTAMPTZ,
    FOREIGN KEY(owner_id, resource_id) REFERENCES managed_resources(owner_id, id),
    FOREIGN KEY(owner_id, resource_id, upstream_base_revision_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    FOREIGN KEY(owner_id, resource_id, managed_candidate_revision_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    FOREIGN KEY(owner_id, resource_id, incoming_revision_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    FOREIGN KEY(owner_id, resource_id, resolved_revision_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    UNIQUE(owner_id, resource_id, managed_candidate_revision_id, incoming_revision_id)
);
CREATE TABLE IF NOT EXISTS managed_reconciliation_conflicts (
    reconciliation_id TEXT NOT NULL REFERENCES managed_reconciliations(id),
    path TEXT NOT NULL,
    base_digest TEXT,
    candidate_digest TEXT,
    incoming_digest TEXT,
    resolution TEXT CHECK (resolution IN ('candidate','incoming','manual','delete')),
    resolved_digest TEXT,
    PRIMARY KEY(reconciliation_id, path)
);
CREATE TABLE IF NOT EXISTS managed_withdrawal_proposals (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    snapshot_id TEXT NOT NULL,
    reason TEXT NOT NULL CHECK (length(reason) BETWEEN 1 AND 4000),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_by TEXT REFERENCES users(id),
    decided_at TIMESTAMPTZ,
    FOREIGN KEY(owner_id, resource_id) REFERENCES managed_resources(owner_id, id),
    FOREIGN KEY(owner_id, snapshot_id) REFERENCES managed_source_snapshots(owner_id, id),
    UNIQUE(owner_id, resource_id, snapshot_id)
);
CREATE TABLE IF NOT EXISTS managed_distribution_deliveries (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    outbox_id TEXT NOT NULL REFERENCES managed_distribution_outbox(id),
    publication_id TEXT NOT NULL,
    generation BIGINT NOT NULL CHECK (generation > 0),
    bundle_digest TEXT,
    status TEXT NOT NULL CHECK (status IN ('claimed','distributed','failed')),
    claim_token TEXT NOT NULL,
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ,
    error TEXT,
    FOREIGN KEY(owner_id, publication_id) REFERENCES managed_publications(owner_id, id),
    UNIQUE(outbox_id),
    UNIQUE(owner_id, claim_token)
);
CREATE TABLE IF NOT EXISTS managed_installation_receipts (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    installation_id TEXT NOT NULL,
    publication_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    generation BIGINT NOT NULL CHECK (generation > 0),
    bundle_digest TEXT NOT NULL CHECK (bundle_digest ~ '^[0-9a-f]{64}$'),
    installed_manifest JSONB NOT NULL,
    client_evidence JSONB NOT NULL,
    verified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id, resource_id, publication_id) REFERENCES managed_publications(owner_id, resource_id, id),
    UNIQUE(owner_id, installation_id, resource_id, generation),
    UNIQUE(owner_id, id)
);
CREATE TABLE IF NOT EXISTS managed_invocation_attributions (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    invocation_id TEXT NOT NULL,
    installation_id TEXT,
    resource_id TEXT,
    revision_id TEXT,
    publication_generation BIGINT,
    traffic_class TEXT NOT NULL CHECK (traffic_class IN ('production','fixture','live_evaluation','suggestion','judge')),
    status TEXT NOT NULL CHECK (status IN ('verified','revision_unknown','unsupported','historical')),
    receipt_id TEXT,
    authenticated_evidence JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id, receipt_id) REFERENCES managed_installation_receipts(owner_id, id),
    UNIQUE(owner_id, invocation_id)
);
DROP TRIGGER IF EXISTS managed_publication_reviews_immutable ON managed_publication_reviews;
CREATE TRIGGER managed_publication_reviews_immutable BEFORE UPDATE ON managed_publication_reviews FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_publications_immutable ON managed_publications;
CREATE TRIGGER managed_publications_immutable BEFORE UPDATE ON managed_publications FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_installation_receipts_immutable ON managed_installation_receipts;
CREATE TRIGGER managed_installation_receipts_immutable BEFORE UPDATE ON managed_installation_receipts FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_invocation_attributions_immutable ON managed_invocation_attributions;
CREATE TRIGGER managed_invocation_attributions_immutable BEFORE UPDATE ON managed_invocation_attributions FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
