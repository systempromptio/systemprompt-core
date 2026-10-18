-- @compat: breaking
-- Records 042_quota_bucket_shards as the contract migration it is: 042 swapped
-- the `ai_quota_buckets_subject_key` constraint for one that includes `shard`,
-- so a binary whose upserts name the four-column conflict target fails every
-- quota admission once 042 has run. 042 shipped without the directive and is
-- immutable; this marker makes `infra db migrate-plan --require-rolling-safe`
-- refuse a batch that carries it and makes later releases refuse to start
-- against a database a newer one contracted.
SELECT 1;
