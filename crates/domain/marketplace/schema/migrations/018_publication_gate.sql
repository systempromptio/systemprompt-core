-- @supersedes-checksum: 370906b6ef8edf56
-- A publication admitted by a publication gate records which gate admitted it
-- and the digest of the evidence that gate returned.
ALTER TABLE managed_publication_reviews ADD COLUMN IF NOT EXISTS gate_id TEXT
    CHECK (gate_id IS NULL OR length(gate_id) BETWEEN 1 AND 200);
ALTER TABLE managed_publication_reviews ADD COLUMN IF NOT EXISTS gate_evidence_digest TEXT
    CHECK (gate_evidence_digest IS NULL OR gate_evidence_digest ~ '^[0-9a-f]{64}$');
ALTER TABLE managed_publication_reviews DROP CONSTRAINT IF EXISTS managed_publication_reviews_gate_pair;
ALTER TABLE managed_publication_reviews ADD CONSTRAINT managed_publication_reviews_gate_pair
    CHECK ((gate_id IS NULL) = (gate_evidence_digest IS NULL)) NOT VALID;
ALTER TABLE managed_publication_reviews VALIDATE CONSTRAINT managed_publication_reviews_gate_pair;
