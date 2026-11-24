-- The boot nonce of the supervising process that spawned a service's child,
-- recorded with its pid. A child is signalled only when its environment
-- carries both the service marker and this nonce. Nullable: a row whose pid
-- was recorded before this column existed keeps NULL until the child is next
-- restarted. Adding a nullable column without a default rewrites nothing.
ALTER TABLE services ADD COLUMN IF NOT EXISTS spawn_nonce TEXT;
