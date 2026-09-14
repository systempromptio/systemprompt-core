CREATE TABLE IF NOT EXISTS analytics_snapshot_dirty (
 owner_id TEXT NOT NULL REFERENCES users(id),scope TEXT NOT NULL,PRIMARY KEY(owner_id,scope)
);
CREATE TABLE IF NOT EXISTS analytics_snapshot_shadow (
 owner_id TEXT NOT NULL REFERENCES users(id), fact_kind TEXT NOT NULL, source TEXT NOT NULL, fact_id TEXT NOT NULL,
 fact JSONB NOT NULL, occurred_at TIMESTAMPTZ NOT NULL, generation BIGINT NOT NULL,
 PRIMARY KEY(owner_id,fact_kind,source,fact_id)
);
CREATE INDEX IF NOT EXISTS analytics_snapshot_shadow_time ON analytics_snapshot_shadow(owner_id,occurred_at);
CREATE TABLE IF NOT EXISTS analytics_snapshot_daily (
 owner_id TEXT NOT NULL REFERENCES users(id), scope TEXT NOT NULL, day DATE NOT NULL,
 metrics JSONB NOT NULL, spend JSONB NOT NULL, histogram JSONB NOT NULL, histogram_version INTEGER NOT NULL DEFAULT 1,
 cohort BIGINT NOT NULL, suppressed BOOLEAN NOT NULL DEFAULT false, generation BIGINT NOT NULL,
 PRIMARY KEY(owner_id,scope,day)
);
CREATE TABLE IF NOT EXISTS analytics_snapshot_identities (
 owner_id TEXT NOT NULL REFERENCES users(id),scope TEXT NOT NULL,day DATE NOT NULL,kind TEXT NOT NULL,identity TEXT NOT NULL,
 PRIMARY KEY(owner_id,scope,day,kind,identity)
);
CREATE TABLE IF NOT EXISTS analytics_snapshot_state (
 owner_id TEXT PRIMARY KEY REFERENCES users(id), generation BIGINT NOT NULL DEFAULT 0, fact_generation BIGINT NOT NULL DEFAULT 0,
 generated_at TIMESTAMPTZ, day DATE, retained_from DATE, compacted_before DATE, evidence_cutoff TIMESTAMPTZ,
 last_error TEXT
);
CREATE TABLE IF NOT EXISTS analytics_feedback_snapshots (
 owner_id TEXT NOT NULL REFERENCES users(id),scope TEXT NOT NULL,window_days INTEGER NOT NULL,
 generation BIGINT NOT NULL,from_day DATE NOT NULL,to_day DATE NOT NULL,body JSONB NOT NULL,
 PRIMARY KEY(owner_id,scope,window_days)
);
CREATE TABLE IF NOT EXISTS analytics_snapshot_jobs (
 owner_id TEXT NOT NULL REFERENCES users(id),job_id TEXT NOT NULL,scope TEXT NOT NULL,from_day DATE NOT NULL,to_day DATE NOT NULL,
 state TEXT NOT NULL DEFAULT 'pending' CHECK(state IN('pending','leased','ready','failed')),
 lease_worker TEXT,lease_epoch BIGINT NOT NULL DEFAULT 0,lease_until TIMESTAMPTZ,result JSONB,last_error TEXT,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),completed_at TIMESTAMPTZ,
 PRIMARY KEY(owner_id,job_id),CHECK(to_day>from_day AND to_day-from_day<=365)
);
CREATE INDEX IF NOT EXISTS analytics_snapshot_jobs_pending ON analytics_snapshot_jobs(owner_id,created_at) WHERE state IN('pending','leased');
ALTER TABLE analytics_fact_deltas ADD COLUMN IF NOT EXISTS occurred_at TIMESTAMPTZ;
UPDATE analytics_fact_deltas d SET occurred_at=COALESCE((after_fact->'value'->>'occurred_at')::timestamptz,(before_fact->'value'->>'occurred_at')::timestamptz,created_at) WHERE occurred_at IS NULL;
ALTER TABLE analytics_fact_deltas ALTER COLUMN occurred_at SET NOT NULL;
CREATE OR REPLACE VIEW analytics_snapshot_dimensions AS
SELECT owner_id,fact_kind,source,fact_id,occurred_at,(occurred_at AT TIME ZONE 'UTC')::date AS day,
 fact->'value' AS value,
 fact->'value'->'consumer'->>'consumer_id' AS consumer_id,
 CASE WHEN fact->'value'->'consumer'->>'status'='authenticated' THEN (fact->'value'->'consumer')::text END AS session_identity,
 fact->'value'->'attribution'->>'resource_id' AS resource_id,
 fact->'value'->'invocation_key'->>'source' AS invocation_source,fact->'value'->'invocation_key'->>'id' AS invocation_id,
 fact->'value'->'request_key'->>'source' AS request_source,fact->'value'->'request_key'->>'id' AS request_id,
 (fact->'value'->>'succeeded')::boolean AS succeeded,
 fact->'value'->'spend'->>'currency' AS currency,(fact->'value'->'spend'->>'amount_micros')::bigint AS amount,
 (fact->'value'->>'input_tokens')::bigint AS input_tokens,(fact->'value'->>'output_tokens')::bigint AS output_tokens,
 (fact->'value'->>'latency_micros')::bigint AS latency,
 fact->'value'->'conversation_key'->>'source' AS conversation_source,fact->'value'->'conversation_key'->>'id' AS conversation_id,
 fact->'value'->'outcome'->>'status' AS assessment_status
FROM analytics_snapshot_shadow;
CREATE OR REPLACE VIEW analytics_snapshot_contributions AS
WITH assessment AS (
 SELECT DISTINCT ON(owner_id,conversation_source,conversation_id) * FROM analytics_snapshot_dimensions WHERE fact_kind='assessment'
 ORDER BY owner_id,conversation_source,conversation_id,occurred_at DESC,source,fact_id
),scoped AS (
 SELECT f.*,s.scope FROM analytics_snapshot_dimensions f CROSS JOIN LATERAL (
  SELECT ''::text AS scope UNION SELECT f.resource_id WHERE f.resource_id IS NOT NULL
 ) s WHERE f.fact_kind='invocation'
 UNION ALL
 SELECT f.*,s.scope FROM analytics_snapshot_dimensions f CROSS JOIN LATERAL (
  SELECT ''::text AS scope UNION SELECT DISTINCT a.resource_id FROM analytics_snapshot_dimensions a
   WHERE a.owner_id=f.owner_id AND a.fact_kind='resource_association' AND a.request_source=f.source AND a.request_id=f.fact_id AND a.resource_id IS NOT NULL
 ) s WHERE f.fact_kind='request'
 UNION ALL
 SELECT f.*,s.scope FROM assessment f CROSS JOIN LATERAL (
  SELECT ''::text AS scope UNION SELECT i.resource_id FROM analytics_snapshot_dimensions i
   WHERE i.owner_id=f.owner_id AND i.fact_kind='invocation' AND i.source=f.invocation_source AND i.fact_id=f.invocation_id AND i.resource_id IS NOT NULL
 ) s
)
SELECT * FROM scoped;
