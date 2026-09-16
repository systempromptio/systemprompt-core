ALTER TABLE eval_suggestions ADD COLUMN operation_key TEXT;
ALTER TABLE eval_suggestions ADD COLUMN operation_digest TEXT;
ALTER TABLE eval_suggestions ADD CONSTRAINT eval_suggestion_operation_pair CHECK ((operation_key IS NULL AND operation_digest IS NULL) OR (operation_key IS NOT NULL AND operation_digest IS NOT NULL AND length(operation_key) BETWEEN 1 AND 200 AND operation_digest ~ '^[0-9a-f]{64}$'));
CREATE UNIQUE INDEX eval_suggestion_operation_unique ON eval_suggestions(owner_id,operation_key) WHERE operation_key IS NOT NULL;
