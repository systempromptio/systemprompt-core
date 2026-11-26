-- @no-transaction
-- @cost: rows=300000 measured=40ms triggers=live
-- Every write path sets `logs.user_id` from a non-optional `UserId`
-- (`LogEntry::user_id`, bound by the single-row insert and the batched
-- UNNEST insert), so a row without one is a contract breach. The schema now
-- says so (schema/log.sql). The file runs outside a transaction so the
-- ACCESS EXCLUSIVE lock of the NOT VALID add is released before VALIDATE scans
-- the table under SHARE UPDATE EXCLUSIVE, which does not block writers. Every
-- statement is idempotent, so a run interrupted part-way replays cleanly.
ALTER TABLE logs DROP CONSTRAINT IF EXISTS logs_user_id_not_null;
ALTER TABLE logs
    ADD CONSTRAINT logs_user_id_not_null CHECK (num_nulls(user_id) = 0) NOT VALID;
ALTER TABLE logs VALIDATE CONSTRAINT logs_user_id_not_null;
