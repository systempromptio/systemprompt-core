-- A delivery pass claims a `pending` row with a lease: a per-claim token and
-- an expiry. Channels run outside any transaction; every write the pass makes
-- afterwards matches the token, and a claim whose lease has expired may be
-- taken again by the sweep. Both columns are nullable with no default.

ALTER TABLE notifications ADD COLUMN IF NOT EXISTS delivery_lease_token UUID;
ALTER TABLE notifications ADD COLUMN IF NOT EXISTS delivery_lease_until TIMESTAMPTZ;
