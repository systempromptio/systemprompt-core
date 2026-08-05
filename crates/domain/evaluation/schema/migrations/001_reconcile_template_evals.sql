-- Databases where the template's prototype eval schema created these tables
-- first lack the columns core added when it took ownership of the eval spine.
ALTER TABLE eval_runs ADD COLUMN IF NOT EXISTS rubric_id TEXT;
ALTER TABLE eval_runs ADD COLUMN IF NOT EXISTS trigger_source TEXT NOT NULL DEFAULT 'manual';

ALTER TABLE eval_cases ADD COLUMN IF NOT EXISTS repair_hint TEXT;
ALTER TABLE eval_cases ADD COLUMN IF NOT EXISTS canonical_messages JSONB;
ALTER TABLE eval_cases ADD COLUMN IF NOT EXISTS system_prompt TEXT;
ALTER TABLE eval_cases ADD COLUMN IF NOT EXISTS offered_tools JSONB;
ALTER TABLE eval_cases ADD COLUMN IF NOT EXISTS provider TEXT;
ALTER TABLE eval_cases ADD COLUMN IF NOT EXISTS model TEXT;
ALTER TABLE eval_cases ADD COLUMN IF NOT EXISTS prepared_body_sha256 TEXT;

ALTER TABLE eval_results ADD COLUMN IF NOT EXISTS repaired BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE eval_results ADD COLUMN IF NOT EXISTS replay_of_result_id TEXT;
ALTER TABLE eval_results ADD COLUMN IF NOT EXISTS judge_ai_request_id TEXT;
CREATE INDEX IF NOT EXISTS idx_eval_results_replay_of ON eval_results(replay_of_result_id);
-- A replay result shares (run_id, ai_request_id) with the result it repairs;
-- uniqueness applies only to the first-pass row.
DROP INDEX IF EXISTS idx_eval_results_run_request;
CREATE UNIQUE INDEX idx_eval_results_run_request
    ON eval_results(run_id, ai_request_id)
    WHERE ai_request_id IS NOT NULL AND replay_of_result_id IS NULL;

ALTER TABLE eval_judge_calls ADD COLUMN IF NOT EXISTS result_id TEXT;
ALTER TABLE eval_judge_calls ADD COLUMN IF NOT EXISTS judge_ai_request_id TEXT;
ALTER TABLE eval_judge_calls ADD COLUMN IF NOT EXISTS rubric_id TEXT;
ALTER TABLE eval_judge_calls ADD COLUMN IF NOT EXISTS cost_microdollars BIGINT NOT NULL DEFAULT 0;
