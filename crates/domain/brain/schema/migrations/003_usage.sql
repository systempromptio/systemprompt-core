-- Usage feeds importance (R6): the brain_usage job writes each fact's
-- Laplace-smoothed cited and ignored shares here, and recall's in-context
-- score reads them. No revision is recorded for these columns.

ALTER TABLE brain_facts
    ADD COLUMN IF NOT EXISTS usage_cited_rate DOUBLE PRECISION NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS usage_ignored_rate DOUBLE PRECISION NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS usage_updated_at TIMESTAMPTZ;
