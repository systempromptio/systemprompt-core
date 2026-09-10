-- Rule provenance. `access_control_entities` has carried a `source` since the
-- catalog split; rules had none, so a bundle-scoped prune could not tell its
-- own rows from a dashboard operator's. Existing rows predate any bundle and
-- came from the baked services tree, which is exactly what 'yaml' names.
ALTER TABLE access_control_rules ADD COLUMN IF NOT EXISTS source TEXT NOT NULL DEFAULT 'yaml';

CREATE INDEX IF NOT EXISTS idx_access_control_rules_source ON access_control_rules(source);
