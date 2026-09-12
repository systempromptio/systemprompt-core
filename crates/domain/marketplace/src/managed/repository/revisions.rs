//! Atomic revision writes bind every file and dependency to the same owner.

use sqlx::types::Json;
use systemprompt_identifiers::{ResourceRevisionId, UserId};

use super::{ManagedRepository, NewRevision};
use crate::managed::error::invalid;
use crate::managed::{
    AssetDigest, AssetFile, ManagedError, Result, RevisionFiles, RevisionManifest,
};

impl ManagedRepository {
    pub async fn create_revision(
        &self,
        owner: &UserId,
        input: &NewRevision,
    ) -> Result<ResourceRevisionId> {
        if input.rationale.trim().is_empty() || input.rationale.len() > 4000 {
            return Err(invalid("Revision rationale requires 1–4000 bytes"));
        }
        let manifest = RevisionManifest::from_files(
            input.snapshot_id.clone(),
            input.parent_id.clone(),
            &input.files,
            input.dependencies.clone(),
        )?;
        let digest = manifest.digest()?;
        let mut tx = self.pool.begin().await?;
        let source = sqlx::query_scalar!(
            "SELECT source_id FROM managed_resources WHERE id=$1 AND owner_id=$2",
            input.resource_id.as_str(),
            owner.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ManagedError::Unavailable)?;
        let valid = sqlx::query_scalar!(
            "SELECT id FROM managed_source_snapshots WHERE id=$1 AND owner_id=$2 AND source_id=$3",
            input.snapshot_id.as_str(),
            owner.as_str(),
            source
        )
        .fetch_optional(&mut *tx)
        .await?;
        if valid.is_none() {
            return Err(ManagedError::Unavailable);
        }
        if let Some(parent) = &input.parent_id {
            let valid = sqlx::query_scalar!(
                "SELECT id FROM managed_revisions WHERE id=$1 AND owner_id=$2 AND resource_id=$3",
                parent.as_str(),
                owner.as_str(),
                input.resource_id.as_str()
            )
            .fetch_optional(&mut *tx)
            .await?;
            if valid.is_none() {
                return Err(ManagedError::Unavailable);
            }
        }
        for dependency in manifest.dependencies.values() {
            let stored = sqlx::query_scalar!(
                "SELECT digest FROM managed_revisions WHERE id=$1 AND owner_id=$2",
                dependency.revision_id.as_str(),
                owner.as_str()
            )
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ManagedError::Unavailable)?;
            if stored != dependency.digest.as_str() {
                return Err(ManagedError::Integrity);
            }
        }
        let id = ResourceRevisionId::generate();
        sqlx::query!("INSERT INTO managed_revisions(id,owner_id,resource_id,source_id,snapshot_id,parent_id,digest,manifest,rationale) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(resource_id,digest) DO NOTHING",
            id.as_str(), owner.as_str(), input.resource_id.as_str(), source, input.snapshot_id.as_str(), input.parent_id.as_ref().map(ResourceRevisionId::as_str), digest.as_str(), Json(&manifest) as _, input.rationale).execute(&mut *tx).await?;
        let stored_id = sqlx::query_scalar!(
            "SELECT id FROM managed_revisions WHERE resource_id=$1 AND owner_id=$2 AND digest=$3",
            input.resource_id.as_str(),
            owner.as_str(),
            digest.as_str()
        )
        .fetch_one(&mut *tx)
        .await?;
        for (path, file) in &input.files.0 {
            let hash = AssetDigest::of(&file.bytes);
            sqlx::query!("INSERT INTO managed_assets(owner_id,digest,content) VALUES($1,$2,$3) ON CONFLICT(owner_id,digest) DO NOTHING",
                owner.as_str(), hash.as_str(), &file.bytes).execute(&mut *tx).await?;
            sqlx::query!("INSERT INTO managed_revision_assets(owner_id,revision_id,path,digest) VALUES($1,$2,$3,$4) ON CONFLICT(revision_id,path) DO NOTHING",
                owner.as_str(), stored_id, path, hash.as_str()).execute(&mut *tx).await?;
        }
        for dependency in manifest.dependencies.values() {
            sqlx::query!("INSERT INTO managed_revision_dependencies(owner_id,revision_id,dependency_id) VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
                owner.as_str(), stored_id, dependency.revision_id.as_str()).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(ResourceRevisionId::new(stored_id))
    }

    pub async fn get_revision(
        &self,
        owner: &UserId,
        id: &ResourceRevisionId,
    ) -> Result<RevisionManifest> {
        let row = sqlx::query!(r#"SELECT digest, manifest AS "manifest!: Json<RevisionManifest>" FROM managed_revisions WHERE id=$1 AND owner_id=$2"#,
            id.as_str(), owner.as_str()).fetch_optional(&self.pool).await?.ok_or(ManagedError::Unavailable)?;
        if row.manifest.0.digest()?.as_str() != row.digest {
            return Err(ManagedError::Integrity);
        }
        Ok(row.manifest.0)
    }

    pub async fn get_revision_files(
        &self,
        owner: &UserId,
        id: &ResourceRevisionId,
    ) -> Result<RevisionFiles> {
        let manifest = self.get_revision(owner, id).await?;
        let rows = sqlx::query!("SELECT r.path,r.digest,a.content FROM managed_revision_assets r JOIN managed_assets a ON a.owner_id=r.owner_id AND a.digest=r.digest WHERE r.owner_id=$1 AND r.revision_id=$2 ORDER BY r.path",
            owner.as_str(), id.as_str()).fetch_all(&self.pool).await?;
        if rows.len() != manifest.files.len() {
            return Err(ManagedError::Integrity);
        }
        let mut files = RevisionFiles::default();
        for row in rows {
            let file = manifest
                .files
                .get(&row.path)
                .ok_or(ManagedError::Integrity)?;
            if file.digest.as_str() != row.digest
                || file.bytes != row.content.len() as u64
                || AssetDigest::of(&row.content) != file.digest
            {
                return Err(ManagedError::Integrity);
            }
            files.0.insert(
                row.path,
                AssetFile {
                    bytes: row.content,
                    media_type: file.media_type.clone(),
                    executable: file.executable,
                },
            );
        }
        files.validate()?;
        Ok(files)
    }
}
