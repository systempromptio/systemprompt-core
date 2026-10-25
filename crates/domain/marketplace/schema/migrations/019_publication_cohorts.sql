-- @supersedes-checksum: d6781c409d871601
-- @supersedes-checksum: 3e09b55c5bf7561b
-- @cost: rows=252 measured=36ms triggers=live
-- Cohort publication: a publication may select a revision for one cohort
-- (a role or a subject-attribute value) instead of the resource's base
-- selection. Reviews record the cohort they targeted.
ALTER TABLE managed_publication_reviews ADD COLUMN IF NOT EXISTS cohort_dimension TEXT;
ALTER TABLE managed_publication_reviews ADD COLUMN IF NOT EXISTS cohort_value TEXT;
ALTER TABLE managed_publication_reviews DROP CONSTRAINT IF EXISTS managed_publication_reviews_cohort_pair;
ALTER TABLE managed_publication_reviews ADD CONSTRAINT managed_publication_reviews_cohort_pair
    CHECK ((cohort_dimension IS NULL) = (cohort_value IS NULL)) NOT VALID;
ALTER TABLE managed_publication_reviews VALIDATE CONSTRAINT managed_publication_reviews_cohort_pair;
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
