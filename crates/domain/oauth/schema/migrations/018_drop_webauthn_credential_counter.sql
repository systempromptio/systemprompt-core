-- The signature counter lives inside the serialised passkey stored in
-- public_key, which authentication reads and refreshes. The separate counter
-- column was written but never read, so it is dropped.
ALTER TABLE webauthn_credentials DROP COLUMN IF EXISTS counter;
