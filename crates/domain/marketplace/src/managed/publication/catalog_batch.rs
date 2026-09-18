//! Set-based reads for the catalogue overlay: every managed skill of one
//! principal resolved in a single query, the revoked keys of one consumer in
//! another, and the published revision closures assembled two queries at a
//! time instead of two per skill.
//!
//! The per-key path (`resolve_managed` → `get_publication_bundle` →
//! `get_revision_bundle`) stays the runtime's authority; this module returns
//! the same states and the same verified bundles, only fetched together.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use sqlx::types::Json;
use systemprompt_identifiers::{ManagedResourceId, ResourceRevisionId, UserId};
use systemprompt_models::managed::{
    ASSEMBLER_VERSION, MAX_BYTES, MAX_FILES, MAX_REVISIONS, RevisionManifest,
};

use super::{
    AssetDigest, ManagedError, ManagedRepository, ManagedResolution, Result, RevisionBundle,
    SelectionRow, resolution_from_row,
};
use crate::managed::error::invalid;

/// One managed skill of a principal with the publication that currently
/// selects it, and — when that publication is a live one — the retained
/// revision the selection pins.
#[derive(Debug, Clone)]
pub struct SkillResolutionRow {
    pub resource_key: String,
    pub resolution: ManagedResolution,
}

impl ManagedRepository {
    pub async fn list_skill_resolutions(&self, owner: &UserId) -> Result<Vec<SkillResolutionRow>> {
        let rows = sqlx::query!(
            r#"SELECT r.id, r.resource_key,
                s.generation AS "generation?", s.state AS "state?", s.publication_id AS "publication_id?",
                s.revision_id AS "revision_id?", s.bundle_digest AS "bundle_digest?",
                p.revision_id AS "published_revision_id?", p.bundle_digest AS "published_bundle_digest?"
            FROM managed_resources r
            LEFT JOIN managed_publication_selections s ON s.owner_id=r.owner_id AND s.resource_id=r.id
            LEFT JOIN managed_publications p ON p.owner_id=s.owner_id AND p.resource_id=s.resource_id
                AND p.generation=s.generation AND p.revision_id IS NOT NULL
            WHERE r.owner_id=$1 AND r.kind='skill'
            ORDER BY r.resource_key, r.id"#,
            owner.as_str()
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                let resource_id = ManagedResourceId::new(row.id);
                let resolution = match (row.generation, row.state, row.publication_id) {
                    (Some(generation), Some(state), Some(publication_id)) => {
                        let resolved = resolution_from_row(
                            resource_id,
                            SelectionRow {
                                generation,
                                state,
                                publication_id,
                                revision_id: row.revision_id,
                                bundle_digest: row.bundle_digest,
                            },
                        )?;
                        // Why: the runtime re-reads the publication row for the
                        // selected generation and refuses a selection it does not
                        // back; the join carries that row so the check is kept.
                        if let ManagedResolution::Published {
                            resource_id,
                            generation,
                            revision_id,
                            bundle_digest,
                            ..
                        } = &resolved
                            && (row.published_revision_id.as_deref() != Some(revision_id.as_str())
                                || row.published_bundle_digest.as_deref()
                                    != Some(bundle_digest.as_str()))
                        {
                            return Ok(SkillResolutionRow {
                                resource_key: row.resource_key,
                                resolution: ManagedResolution::IntegrityFailure {
                                    resource_id: resource_id.clone(),
                                    generation: *generation,
                                },
                            });
                        }
                        resolved
                    },
                    _ => ManagedResolution::NeverAdopted { resource_id },
                };
                Ok(SkillResolutionRow {
                    resource_key: row.resource_key,
                    resolution,
                })
            })
            .collect()
    }

    pub async fn managed_catalog_stamp(&self, owner: &UserId, consumer: &UserId) -> Result<String> {
        let principals = vec![owner.as_str().to_owned(), consumer.as_str().to_owned()];
        let stamp = sqlx::query_scalar!(
            r#"SELECT md5(concat_ws('|',
                (SELECT count(*)::text || ':' || coalesce(max(created_at)::text, '') FROM managed_resources WHERE owner_id = ANY($1) AND kind='skill'),
                (SELECT count(*)::text || ':' || coalesce(sum(generation)::text, '') || ':' || coalesce(max(updated_at)::text, '') FROM managed_publication_selections WHERE owner_id = ANY($1)),
                (SELECT count(revoked_at)::text || ':' || coalesce(max(revoked_at)::text, '') FROM managed_consumer_grants WHERE owner_id=$2 AND consumer_id=$3)
            )) AS "stamp!""#,
            &principals,
            owner.as_str(),
            consumer.as_str()
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(stamp)
    }

    pub async fn revoked_skill_keys(
        &self,
        owner: &UserId,
        consumer: &UserId,
    ) -> Result<BTreeSet<String>> {
        let keys = sqlx::query_scalar!(
            "SELECT r.resource_key FROM managed_consumer_grants g JOIN managed_resources r ON r.id=g.resource_id AND r.owner_id=g.owner_id WHERE g.owner_id=$1 AND g.consumer_id=$2 AND r.kind='skill' AND g.revoked_at IS NOT NULL",
            owner.as_str(),
            consumer.as_str()
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(keys.into_iter().collect())
    }

    pub async fn get_revision_bundles(
        &self,
        roots: &[(UserId, ResourceRevisionId)],
    ) -> Result<BTreeMap<ResourceRevisionId, RevisionBundle>> {
        let mut manifests: BTreeMap<(String, String), RevisionManifest> = BTreeMap::new();
        let mut assets: BTreeMap<(String, String), Vec<FetchedAsset>> = BTreeMap::new();
        let mut pending: BTreeSet<(String, String)> = roots
            .iter()
            .map(|(owner, id)| (owner.as_str().to_owned(), id.as_str().to_owned()))
            .collect();
        while !pending.is_empty() {
            let owners: Vec<String> = pending.iter().map(|(owner, _)| owner.clone()).collect();
            let ids: Vec<String> = pending.iter().map(|(_, id)| id.clone()).collect();
            let wanted = std::mem::take(&mut pending);
            let manifest_rows = sqlx::query!(
                r#"SELECT owner_id, id, digest, manifest AS "manifest!: Json<RevisionManifest>" FROM managed_revisions WHERE owner_id = ANY($1) AND id = ANY($2)"#,
                &owners,
                &ids
            )
            .fetch_all(&self.pool)
            .await?;
            let asset_rows = sqlx::query!(
                "SELECT r.owner_id, r.revision_id, r.path, r.digest, a.content FROM managed_revision_assets r JOIN managed_assets a ON a.owner_id=r.owner_id AND a.digest=r.digest WHERE r.owner_id = ANY($1) AND r.revision_id = ANY($2) ORDER BY r.path",
                &owners,
                &ids
            )
            .fetch_all(&self.pool)
            .await?;
            for row in asset_rows {
                let key = (row.owner_id, row.revision_id);
                if wanted.contains(&key) {
                    assets.entry(key).or_default().push(FetchedAsset {
                        path: row.path,
                        digest: row.digest,
                        content: row.content,
                    });
                }
            }
            for row in manifest_rows {
                let key = (row.owner_id, row.id);
                if !wanted.contains(&key) {
                    continue;
                }
                if row.manifest.0.digest()?.as_str() != row.digest {
                    return Err(ManagedError::Integrity);
                }
                for dependency in row.manifest.0.dependencies.values() {
                    let next = (key.0.clone(), dependency.revision_id.as_str().to_owned());
                    if !manifests.contains_key(&next) {
                        pending.insert(next);
                    }
                }
                manifests.insert(key, row.manifest.0);
            }
        }

        let mut bundles = BTreeMap::new();
        for (owner, root) in roots {
            let bundle = assemble_bundle(owner, root, &manifests, &assets)?;
            bundles.insert(root.clone(), bundle);
        }
        Ok(bundles)
    }
}

struct FetchedAsset {
    path: String,
    digest: String,
    content: Vec<u8>,
}

fn assemble_bundle(
    owner: &UserId,
    root: &ResourceRevisionId,
    manifests: &BTreeMap<(String, String), RevisionManifest>,
    assets: &BTreeMap<(String, String), Vec<FetchedAsset>>,
) -> Result<RevisionBundle> {
    let mut bundle = RevisionBundle {
        schema_version: 1,
        assembler_version: ASSEMBLER_VERSION.to_owned(),
        root: root.clone(),
        revisions: BTreeMap::new(),
        assets: BTreeMap::new(),
    };
    let mut pending = vec![root.clone()];
    let mut file_count = 0usize;
    let mut expanded_bytes = 0usize;
    while let Some(id) = pending.pop() {
        if bundle.revisions.contains_key(&id) {
            continue;
        }
        if bundle.revisions.len() >= MAX_REVISIONS {
            return Err(invalid("Bundle exceeds 64 revisions"));
        }
        let key = (owner.as_str().to_owned(), id.as_str().to_owned());
        let manifest = manifests.get(&key).ok_or(ManagedError::Unavailable)?;
        let files = assets.get(&key).map_or(&[][..], Vec::as_slice);
        if files.len() != manifest.files.len() {
            return Err(ManagedError::Integrity);
        }
        file_count += files.len();
        if file_count > MAX_FILES {
            return Err(invalid("Bundle exceeds 256 files"));
        }
        for file in files {
            let expected = manifest
                .files
                .get(&file.path)
                .ok_or(ManagedError::Integrity)?;
            if expected.digest.as_str() != file.digest
                || expected.bytes != file.content.len() as u64
                || AssetDigest::of(&file.content) != expected.digest
            {
                return Err(ManagedError::Integrity);
            }
            expanded_bytes = expanded_bytes
                .checked_add(file.content.len())
                .ok_or(ManagedError::Integrity)?;
            if expanded_bytes > MAX_BYTES {
                return Err(invalid("Bundle exceeds 8 MiB expanded content"));
            }
            bundle
                .assets
                .entry(expected.digest.clone())
                .or_insert_with(|| file.content.clone());
        }
        pending.extend(
            manifest
                .dependencies
                .values()
                .map(|dependency| dependency.revision_id.clone()),
        );
        bundle.revisions.insert(id, manifest.clone());
    }
    bundle.verify()?;
    Ok(bundle)
}
