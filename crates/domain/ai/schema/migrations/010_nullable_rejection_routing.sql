-- A request rejected before routing has no provider and no model, but both
-- columns were NOT NULL, so the rejection path wrote the literal 'unknown'.
-- Those rows are indistinguishable from a real request to a provider named
-- 'unknown' by anything reading the spine, and every per-provider aggregate
-- had to string-match a magic value to exclude them.
ALTER TABLE ai_requests ALTER COLUMN provider DROP NOT NULL;
ALTER TABLE ai_requests ALTER COLUMN model DROP NOT NULL;

UPDATE ai_requests SET status = 'rejected'
 WHERE provider = 'unknown' OR model = 'unknown';
UPDATE ai_requests SET provider = NULL WHERE provider = 'unknown';
UPDATE ai_requests SET model = NULL WHERE model = 'unknown';

-- Nullable is only correct for a request refused before routing resolved a
-- provider. Anything that reached a provider must carry both columns, so the
-- invariant moves from NOT NULL to a status-keyed constraint rather than being
-- dropped. Provider and model resolve independently, so this cannot be
-- expressed as `(provider IS NULL) = (model IS NULL)`.
ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_routed_has_provider;
ALTER TABLE ai_requests ADD CONSTRAINT ai_requests_routed_has_provider
  CHECK (status = 'rejected' OR (provider IS NOT NULL AND model IS NOT NULL));
