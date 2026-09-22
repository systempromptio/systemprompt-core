-- A successful run's own report (`JobResult::message`), so the console can
-- show what a retention pass deleted without a run-history table.
ALTER TABLE scheduled_jobs ADD COLUMN IF NOT EXISTS last_message TEXT;
