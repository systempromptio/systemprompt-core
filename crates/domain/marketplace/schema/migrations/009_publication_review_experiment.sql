-- Improvement reviews bind to the attested experiment by a typed column rather
-- than a key inside the reviewer's free-form evidence document.
ALTER TABLE managed_publication_reviews ADD COLUMN IF NOT EXISTS experiment_id TEXT;
