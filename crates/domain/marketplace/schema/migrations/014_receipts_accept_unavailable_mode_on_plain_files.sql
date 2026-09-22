-- A receipt was fully verified only when every file's mode check passed, and
-- a host without POSIX mode bits (Windows) reports the mode as unavailable,
-- so no installation from such a host ever verified: it counted for nothing
-- in adoption and every invocation on it was attributed as revision_unknown.
-- A file that is not meant to be executable has no mode to satisfy; the
-- content digest is the whole check. Re-judge the stored evidence by that
-- rule. Receipts are otherwise immutable; the trigger is lifted only for this
-- rewrite, and only where it exists.
DO $$
DECLARE
    guarded BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgname = 'managed_installation_receipts_immutable'
          AND tgrelid = 'managed_installation_receipts'::regclass
    ) INTO guarded;
    IF guarded THEN
        ALTER TABLE managed_installation_receipts DISABLE TRIGGER managed_installation_receipts_immutable;
    END IF;

    UPDATE managed_installation_receipts r
    SET fully_verified = true
    WHERE NOT COALESCE(r.fully_verified, false)
      AND r.consumer_evidence IS NOT NULL
      AND jsonb_array_length(COALESCE(r.consumer_evidence->'runtime_files', '[]'::jsonb)) > 0
      AND NOT EXISTS (
          SELECT 1
          FROM jsonb_array_elements(COALESCE(r.consumer_evidence->'files', '[]'::jsonb)
                                    || COALESCE(r.consumer_evidence->'runtime_files', '[]'::jsonb)) f
          WHERE f->>'content_check' IS DISTINCT FROM 'verified'
             OR NOT (f->>'mode_check' = 'verified'
                     OR (f->>'mode_check' = 'unavailable' AND NOT COALESCE((f->>'executable')::boolean, false)))
      );

    IF guarded THEN
        ALTER TABLE managed_installation_receipts ENABLE TRIGGER managed_installation_receipts_immutable;
    END IF;
END
$$;
