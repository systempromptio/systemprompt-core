CREATE TABLE IF NOT EXISTS managed_publication_cohort_selections (
    owner_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    cohort_dimension TEXT NOT NULL CHECK (length(cohort_dimension) BETWEEN 1 AND 200),
    cohort_value TEXT NOT NULL CHECK (length(cohort_value) BETWEEN 1 AND 200),
    generation BIGINT NOT NULL CHECK (generation > 0),
    publication_id TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    bundle_digest TEXT NOT NULL CHECK (bundle_digest ~ '^[0-9a-f]{64}$'),
    state TEXT NOT NULL CHECK (state = 'published'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(owner_id, resource_id, cohort_dimension, cohort_value),
    FOREIGN KEY(owner_id, resource_id, publication_id) REFERENCES managed_publications(owner_id, resource_id, id),
    FOREIGN KEY(owner_id, resource_id, revision_id) REFERENCES managed_revisions(owner_id, resource_id, id)
);
