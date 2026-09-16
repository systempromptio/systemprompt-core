ALTER TABLE eval_campaigns
    ADD COLUMN IF NOT EXISTS publication_generation BIGINT;

ALTER TABLE eval_campaigns
    ADD COLUMN IF NOT EXISTS composed_hash TEXT;
