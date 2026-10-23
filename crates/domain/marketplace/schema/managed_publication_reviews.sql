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
    gate_id TEXT CHECK (gate_id IS NULL OR length(gate_id) BETWEEN 1 AND 200),
    gate_evidence_digest TEXT CHECK (gate_evidence_digest IS NULL OR gate_evidence_digest ~ '^[0-9a-f]{64}$'),
    cohort_dimension TEXT,
    cohort_value TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id, resource_id) REFERENCES managed_resources(owner_id, id),
    FOREIGN KEY(owner_id, resource_id, revision_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    UNIQUE(owner_id, id),
    CONSTRAINT managed_publication_reviews_gate_pair CHECK ((gate_id IS NULL) = (gate_evidence_digest IS NULL)),
    CONSTRAINT managed_publication_reviews_cohort_pair CHECK ((cohort_dimension IS NULL) = (cohort_value IS NULL))
);
DROP TRIGGER IF EXISTS managed_publication_reviews_immutable ON managed_publication_reviews;
CREATE TRIGGER managed_publication_reviews_immutable BEFORE UPDATE ON managed_publication_reviews FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
