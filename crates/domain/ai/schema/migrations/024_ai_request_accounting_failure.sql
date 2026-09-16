ALTER TABLE ai_requests ADD COLUMN IF NOT EXISTS accounting_failed_at TIMESTAMPTZ;
ALTER TABLE ai_requests ADD COLUMN IF NOT EXISTS accounting_error TEXT;
