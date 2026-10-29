-- @no-transaction
-- Users are selected by role membership with `roles @> ARRAY[...]`.
--
-- Why: the index is built CONCURRENTLY so writes to users continue while it
-- builds, which is only legal outside a transaction. A build interrupted
-- part-way leaves an invalid index behind, so a replay drops it before
-- building again.
DROP INDEX CONCURRENTLY IF EXISTS idx_users_roles_gin;
CREATE INDEX CONCURRENTLY idx_users_roles_gin ON users USING GIN (roles);
