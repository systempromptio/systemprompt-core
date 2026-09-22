-- `logs.level` carried the same CHECK twice: once inline on the column
-- (auto-named `logs_level_check`) and once as the named `log_level_check`.
-- The base schema now declares only the named one; established databases
-- drop the twin so every install ends with the same single constraint.
ALTER TABLE logs DROP CONSTRAINT IF EXISTS logs_level_check;
