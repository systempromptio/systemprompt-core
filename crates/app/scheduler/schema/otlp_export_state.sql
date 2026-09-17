-- Per-signal progress of the OTLP exporter (`otlp_export` job).
--
-- One row per exported signal. `watermark` / `watermark_id` are the
-- (timestamp, id) cursor of the last row shipped, so a tail resumes exactly
-- where it stopped and a batch that fails leaves the cursor untouched. The
-- row is created at first export with the watermark at that moment: enabling
-- the exporter ships new rows forward from then, never the whole history.
-- Deleting a row restarts that signal from now.
CREATE TABLE IF NOT EXISTS otlp_export_state (
    signal TEXT PRIMARY KEY CHECK (signal IN ('traces', 'logs')),
    watermark TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    watermark_id TEXT NOT NULL DEFAULT '',
    last_attempt_at TIMESTAMPTZ,
    last_success_at TIMESTAMPTZ,
    last_error TEXT,
    last_error_at TIMESTAMPTZ,
    batches_total BIGINT NOT NULL DEFAULT 0,
    failures_total BIGINT NOT NULL DEFAULT 0,
    rows_total BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
