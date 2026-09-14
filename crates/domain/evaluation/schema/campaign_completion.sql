CREATE TABLE IF NOT EXISTS eval_campaign_diagnostics (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    campaign_id TEXT REFERENCES eval_campaigns(id),
    operation_key TEXT NOT NULL,
    stage TEXT NOT NULL,
    code TEXT NOT NULL,
    remediation TEXT NOT NULL,
    actor_id TEXT NOT NULL REFERENCES users(id),
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    occurrences BIGINT NOT NULL DEFAULT 1,
    resolved_at TIMESTAMPTZ,
    UNIQUE(owner_id,operation_key,stage,code)
);
CREATE INDEX IF NOT EXISTS eval_campaign_diagnostics_cursor ON eval_campaign_diagnostics(owner_id,campaign_id,id);
CREATE TABLE IF NOT EXISTS eval_campaign_holdout_proposals (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    campaign_id TEXT NOT NULL REFERENCES eval_campaigns(id),
    development_experiment_id TEXT NOT NULL REFERENCES eval_experiments(id),
    operation_key TEXT NOT NULL,
    spec JSONB NOT NULL,
    spec_digest TEXT NOT NULL,
    development_cases INTEGER NOT NULL,
    holdout_cases INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    confirmed_by TEXT REFERENCES users(id),
    confirmed_at TIMESTAMPTZ,
    experiment_id TEXT REFERENCES eval_experiments(id),
    UNIQUE(owner_id,campaign_id,operation_key)
);
CREATE TABLE IF NOT EXISTS eval_holdout_content_consumption (
    owner_id TEXT NOT NULL REFERENCES users(id),
    content_digest TEXT NOT NULL,
    experiment_id TEXT NOT NULL REFERENCES eval_experiments(id),
    PRIMARY KEY(owner_id,content_digest)
);
CREATE OR REPLACE FUNCTION protect_campaign_holdout_proposal()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF (to_jsonb(NEW)-ARRAY['confirmed_by','confirmed_at','experiment_id']) IS DISTINCT FROM
       (to_jsonb(OLD)-ARRAY['confirmed_by','confirmed_at','experiment_id'])
       OR (OLD.confirmed_by IS NOT NULL AND NEW.confirmed_by IS DISTINCT FROM OLD.confirmed_by)
       OR (OLD.confirmed_at IS NOT NULL AND NEW.confirmed_at IS DISTINCT FROM OLD.confirmed_at)
       OR (OLD.experiment_id IS NOT NULL AND NEW.experiment_id IS DISTINCT FROM OLD.experiment_id) THEN
        RAISE EXCEPTION 'Retained holdout proposal is immutable' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE OR REPLACE TRIGGER eval_campaign_holdout_immutable BEFORE UPDATE ON eval_campaign_holdout_proposals
FOR EACH ROW EXECUTE FUNCTION protect_campaign_holdout_proposal();
