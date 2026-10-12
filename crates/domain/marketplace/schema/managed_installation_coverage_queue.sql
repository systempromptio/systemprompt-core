-- Installation coverage as a projection (store, don't compute): one body per
-- (owner, resource) in `managed_installation_coverage`, rebuilt per owner by
-- `refresh_managed_installation_coverage(keys)` where a key is an owner id.
--
-- Coverage depends on receipts, grants, publication selections and the
-- consumers' enrolled devices, so statement triggers on those four tables
-- queue the owners each statement touched on the `managed_installation_coverage`
-- projection; the `managed_inventory_refresh` job drains it, instead of
-- rebuilding every owner's coverage on every tick. A device change reaches
-- every owner that granted that consumer something. The rebuild writes only
-- the bodies that changed, and the owner's generation and `observed_at`
-- (`managed_installation_coverage_state`) move only when at least one did.
-- Migration 017 queues every owner once.
CREATE OR REPLACE FUNCTION refresh_managed_installation_coverage(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE
    owner TEXT;
    gen BIGINT;
    changed BIGINT;
    written BIGINT := 0;
BEGIN
    FOREACH owner IN ARRAY keys LOOP
        INSERT INTO managed_installation_coverage_state(owner_id) VALUES (owner) ON CONFLICT DO NOTHING;
        SELECT generation + 1 INTO gen FROM managed_installation_coverage_state
        WHERE owner_id = owner FOR UPDATE;
        WITH latest AS (
            SELECT DISTINCT ON (resource_id, consumer_id, device_id, host, installation_id) *
            FROM managed_installation_receipts
            WHERE owner_id = owner AND consumer_id IS NOT NULL
            ORDER BY resource_id, consumer_id, device_id, host, installation_id, generation DESC, verified_at DESC
        ), eligible AS (
            SELECT g.resource_id, d.device_id, g.consumer_id
            FROM managed_consumer_grants g
            CROSS JOIN LATERAL public.active_devices_for_consumer(g.consumer_id) d
            WHERE g.owner_id = owner AND g.revoked_at IS NULL
        ), current_receipts AS (
            SELECT l.* FROM latest l
            JOIN managed_publication_selections p
              ON p.owner_id = l.owner_id AND p.resource_id = l.resource_id
             AND p.generation = l.generation AND p.state = 'published'
            JOIN eligible e
              ON e.resource_id = l.resource_id AND e.device_id = l.device_id AND e.consumer_id = l.consumer_id
        ), upserted AS (
            INSERT INTO managed_installation_coverage (owner_id, resource_id, body, generation)
            SELECT owner, r.id, jsonb_build_object(
                'eligible_devices', (SELECT COUNT(DISTINCT device_id) FROM eligible e WHERE e.resource_id = r.id),
                'current_acknowledged_devices', (SELECT COUNT(DISTINCT device_id) FROM current_receipts c WHERE c.resource_id = r.id),
                'current_verified_devices', (SELECT COUNT(DISTINCT device_id) FROM current_receipts c WHERE c.resource_id = r.id AND fully_verified),
                'acknowledged_installations', (SELECT COUNT(*) FROM latest l WHERE l.resource_id = r.id),
                'unverifiable_installations', (SELECT COUNT(*) FROM latest l WHERE l.resource_id = r.id AND NOT COALESCE(fully_verified, false)),
                'legacy_receipts', (SELECT COUNT(*) FROM managed_installation_receipts l WHERE l.owner_id = owner AND l.resource_id = r.id AND l.consumer_id IS NULL)
            ), gen
            FROM managed_resources r WHERE r.owner_id = owner
            ON CONFLICT (owner_id, resource_id) DO UPDATE
            SET body = EXCLUDED.body, generation = EXCLUDED.generation
            WHERE managed_installation_coverage.body IS DISTINCT FROM EXCLUDED.body
            RETURNING 1
        )
        SELECT COUNT(*) INTO changed FROM upserted;
        IF changed > 0 THEN
            UPDATE managed_installation_coverage_state
            SET generation = gen, observed_at = clock_timestamp()
            WHERE owner_id = owner;
        END IF;
        written := written + changed;
    END LOOP;
    RETURN written;
END $$;

CREATE OR REPLACE FUNCTION queue_installation_coverage_owners()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE keys TEXT[];
BEGIN
    IF TG_OP = 'INSERT' THEN
        keys := ARRAY(SELECT DISTINCT n.owner_id FROM new_rows n);
    ELSIF TG_OP = 'DELETE' THEN
        keys := ARRAY(SELECT DISTINCT o.owner_id FROM old_rows o);
    ELSE
        keys := ARRAY(SELECT n.owner_id FROM new_rows n UNION SELECT o.owner_id FROM old_rows o);
    END IF;
    IF cardinality(keys) > 0 THEN
        PERFORM queue_projection_backlog('managed_installation_coverage', keys);
    END IF;
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION queue_installation_coverage_devices()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE consumers TEXT[]; keys TEXT[];
BEGIN
    IF TG_OP = 'INSERT' THEN
        consumers := ARRAY(SELECT DISTINCT n.user_id FROM new_rows n);
    ELSIF TG_OP = 'DELETE' THEN
        consumers := ARRAY(SELECT DISTINCT o.user_id FROM old_rows o);
    ELSE
        consumers := ARRAY(SELECT n.user_id FROM new_rows n UNION SELECT o.user_id FROM old_rows o);
    END IF;
    keys := ARRAY(SELECT DISTINCT g.owner_id FROM managed_consumer_grants g WHERE g.consumer_id = ANY(consumers));
    IF cardinality(keys) > 0 THEN
        PERFORM queue_projection_backlog('managed_installation_coverage', keys);
    END IF;
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS coverage_queue_receipts_insert ON managed_installation_receipts;
CREATE TRIGGER coverage_queue_receipts_insert AFTER INSERT ON managed_installation_receipts
    REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION queue_installation_coverage_owners();
DROP TRIGGER IF EXISTS coverage_queue_receipts_update ON managed_installation_receipts;
CREATE TRIGGER coverage_queue_receipts_update AFTER UPDATE ON managed_installation_receipts
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION queue_installation_coverage_owners();
DROP TRIGGER IF EXISTS coverage_queue_receipts_delete ON managed_installation_receipts;
CREATE TRIGGER coverage_queue_receipts_delete AFTER DELETE ON managed_installation_receipts
    REFERENCING OLD TABLE AS old_rows FOR EACH STATEMENT EXECUTE FUNCTION queue_installation_coverage_owners();

DROP TRIGGER IF EXISTS coverage_queue_grants_insert ON managed_consumer_grants;
CREATE TRIGGER coverage_queue_grants_insert AFTER INSERT ON managed_consumer_grants
    REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION queue_installation_coverage_owners();
DROP TRIGGER IF EXISTS coverage_queue_grants_update ON managed_consumer_grants;
CREATE TRIGGER coverage_queue_grants_update AFTER UPDATE ON managed_consumer_grants
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION queue_installation_coverage_owners();
DROP TRIGGER IF EXISTS coverage_queue_grants_delete ON managed_consumer_grants;
CREATE TRIGGER coverage_queue_grants_delete AFTER DELETE ON managed_consumer_grants
    REFERENCING OLD TABLE AS old_rows FOR EACH STATEMENT EXECUTE FUNCTION queue_installation_coverage_owners();

DROP TRIGGER IF EXISTS coverage_queue_selections_insert ON managed_publication_selections;
CREATE TRIGGER coverage_queue_selections_insert AFTER INSERT ON managed_publication_selections
    REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION queue_installation_coverage_owners();
DROP TRIGGER IF EXISTS coverage_queue_selections_update ON managed_publication_selections;
CREATE TRIGGER coverage_queue_selections_update AFTER UPDATE ON managed_publication_selections
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION queue_installation_coverage_owners();
DROP TRIGGER IF EXISTS coverage_queue_selections_delete ON managed_publication_selections;
CREATE TRIGGER coverage_queue_selections_delete AFTER DELETE ON managed_publication_selections
    REFERENCING OLD TABLE AS old_rows FOR EACH STATEMENT EXECUTE FUNCTION queue_installation_coverage_owners();

DROP TRIGGER IF EXISTS coverage_queue_devices_insert ON user_device_certs;
CREATE TRIGGER coverage_queue_devices_insert AFTER INSERT ON user_device_certs
    REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION queue_installation_coverage_devices();
DROP TRIGGER IF EXISTS coverage_queue_devices_update ON user_device_certs;
CREATE TRIGGER coverage_queue_devices_update AFTER UPDATE ON user_device_certs
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION queue_installation_coverage_devices();
DROP TRIGGER IF EXISTS coverage_queue_devices_delete ON user_device_certs;
CREATE TRIGGER coverage_queue_devices_delete AFTER DELETE ON user_device_certs
    REFERENCING OLD TABLE AS old_rows FOR EACH STATEMENT EXECUTE FUNCTION queue_installation_coverage_devices();
