-- Outcome of the report-only licence check-in (`licence_checkin` job).
--
-- A single row, created at the first attempt. A failed check-in records its
-- error and keeps the last success, so `admin licence status` shows both.
CREATE TABLE IF NOT EXISTS licence_checkin_state (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id),
    last_attempt_at TIMESTAMPTZ,
    last_success_at TIMESTAMPTZ,
    last_status INTEGER,
    last_error TEXT,
    last_error_at TIMESTAMPTZ,
    attempts_total BIGINT NOT NULL DEFAULT 0,
    failures_total BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
