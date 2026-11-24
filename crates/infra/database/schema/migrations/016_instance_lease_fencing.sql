-- Instance lease fencing (schema/instance_leases.sql): every claim that changes
-- a lease's owner stamps the next value of one sequence, so a holder that lost
-- its lease holds a smaller token than its replacement and a fenced write
-- under the old token changes nothing. Rows that predate the column carry 0;
-- the next owner change stamps a sequence value, which is always larger.
CREATE SEQUENCE IF NOT EXISTS instance_lease_fencing_seq;

ALTER TABLE instance_leases ADD COLUMN IF NOT EXISTS fencing_token BIGINT NOT NULL DEFAULT 0;
