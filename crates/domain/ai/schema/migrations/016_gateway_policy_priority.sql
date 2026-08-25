-- Give gateway policies an explicit merge order.
--
-- The effective policy is built by folding rows in ascending order with
-- last-write-wins per section; ordering previously fell back to the name
-- column, which forced naming conventions (`zz-` prefixes) to control
-- precedence. Higher priority now merges later and wins; ties still order
-- by name.

ALTER TABLE ai_gateway_policies
    ADD COLUMN IF NOT EXISTS priority INTEGER NOT NULL DEFAULT 0;
