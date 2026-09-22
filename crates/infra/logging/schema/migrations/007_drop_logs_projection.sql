-- `logs` is no longer a reporting source: at half a million rows it was the
-- bulk of every baseline rebuild and one outbox fact per log line, with a
-- single reader that now groups on agent task error messages instead. The
-- declarative phase no longer defines the view or the trigger.
DROP TRIGGER IF EXISTS reporting_capture ON logs;
DROP VIEW IF EXISTS reporting_source_logs;
