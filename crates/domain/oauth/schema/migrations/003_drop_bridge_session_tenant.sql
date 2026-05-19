-- Remove runtime data-tenancy from bridge_sessions. The tenant_id column was
-- only ever written with the JWT issuer as a placeholder, never a real tenant.

ALTER TABLE bridge_sessions DROP COLUMN IF EXISTS tenant_id;
