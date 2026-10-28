-- Licence check-in state (schema/licence_checkin_state.sql): the outcome of
-- the report-only `licence_checkin` job. The table is the base schema's text.
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
