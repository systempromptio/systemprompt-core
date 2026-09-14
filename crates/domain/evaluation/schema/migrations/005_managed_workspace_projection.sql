-- Replace the legacy text-only workspace authority with retained managed bundle references.
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE TABLE IF NOT EXISTS eval_managed_workspace_projections (
    owner_id TEXT NOT NULL,
    digest TEXT NOT NULL CHECK(digest ~ '^[0-9a-f]{64}$'),
    managed_revision_id TEXT NOT NULL,
    publication_generation BIGINT,
    manifest JSONB NOT NULL,
    verified_file_count INTEGER NOT NULL CHECK(verified_file_count BETWEEN 0 AND 256),
    verified_byte_count BIGINT NOT NULL CHECK(verified_byte_count BETWEEN 0 AND 8388608),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(owner_id,digest)
);
CREATE TABLE IF NOT EXISTS eval_managed_workspace_assets (
    owner_id TEXT NOT NULL,
    workspace_digest TEXT NOT NULL CHECK(workspace_digest ~ '^[0-9a-f]{64}$'),
    path TEXT NOT NULL,
    asset_digest TEXT NOT NULL CHECK(asset_digest ~ '^[0-9a-f]{64}$'),
    content BYTEA NOT NULL,
    executable BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY(owner_id,workspace_digest,path),
    FOREIGN KEY(owner_id,workspace_digest) REFERENCES eval_managed_workspace_projections(owner_id,digest)
);
DO $$
DECLARE legacy_count BIGINT;
DECLARE converted_count BIGINT;
BEGIN
    IF to_regclass('eval_frozen_workspaces') IS NOT NULL THEN
        SELECT count(*) INTO legacy_count FROM eval_frozen_workspaces;
        INSERT INTO eval_managed_workspace_projections(
            owner_id,digest,managed_revision_id,manifest,verified_file_count,verified_byte_count
        )
        SELECT owner_id,digest,'legacy-archive:' || digest,content,
               (SELECT count(*) FROM jsonb_object_keys(content->'files')),
               COALESCE((SELECT sum(octet_length(value #>> '{}')) FROM jsonb_each(content->'files')),0)
        FROM eval_frozen_workspaces
        ON CONFLICT(owner_id,digest) DO NOTHING;
        INSERT INTO eval_managed_workspace_assets(owner_id,workspace_digest,path,asset_digest,content,executable)
        SELECT f.owner_id,f.digest,file.key,
               encode(digest(convert_to(file.value #>> '{}','UTF8'),'sha256'),'hex'),
               convert_to(file.value #>> '{}','UTF8'),FALSE
        FROM eval_frozen_workspaces f
        CROSS JOIN LATERAL jsonb_each(f.content->'files') file
        ON CONFLICT(owner_id,workspace_digest,path) DO NOTHING;
        SELECT count(*) INTO converted_count
        FROM eval_managed_workspace_projections p
        WHERE EXISTS (
            SELECT 1 FROM eval_frozen_workspaces f
            WHERE f.owner_id=p.owner_id AND f.digest=p.digest AND p.manifest=f.content
              AND p.verified_file_count=(SELECT count(*) FROM eval_managed_workspace_assets a WHERE a.owner_id=f.owner_id AND a.workspace_digest=f.digest)
              AND p.verified_byte_count=(SELECT COALESCE(sum(octet_length(a.content)),0) FROM eval_managed_workspace_assets a WHERE a.owner_id=f.owner_id AND a.workspace_digest=f.digest)
              AND NOT EXISTS(SELECT 1 FROM eval_managed_workspace_assets a WHERE a.owner_id=f.owner_id AND a.workspace_digest=f.digest AND a.asset_digest<>encode(digest(a.content,'sha256'),'hex'))
        );
        IF converted_count <> legacy_count THEN
            RAISE EXCEPTION 'legacy workspace conversion count or manifest verification failed';
        END IF;
    END IF;
END $$;
UPDATE eval_experiments e SET status='blocked'
WHERE status IN ('queued','running') AND EXISTS (
    SELECT 1 FROM eval_managed_workspace_projections p
    WHERE p.owner_id=e.owner_id AND p.managed_revision_id LIKE 'legacy-archive:%'
      AND EXISTS (SELECT 1 FROM jsonb_array_elements(e.spec->'variants') v WHERE v->>'skill_bundle_digest'=p.digest OR v->>'configuration_digest'=p.digest)
);
DROP TABLE IF EXISTS eval_frozen_workspaces;
