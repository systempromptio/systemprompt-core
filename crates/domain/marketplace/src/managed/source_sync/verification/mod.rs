//! Complete server-side Git verification of immutable dependency closures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use systemprompt_identifiers::{ManagedResourceId, ManagedSourceId, ResourceRevisionId, UserId};
use systemprompt_models::feedback::verification::{
    DependencyVerificationInput, DependencyVerificationManifest, DependencyVerificationRequest,
};

use super::git::{GitCheckout, import_tree};
use crate::managed::{AssetDigest, ManagedError, ManagedRepository, Result, RevisionFiles};

mod service;
pub use service::GitVerificationService;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitContentVerification {
    pub revision_id: ResourceRevisionId,
    pub source_id: ManagedSourceId,
    pub upstream_root: String,
    pub commit: String,
}

#[derive(Clone, Copy)]
pub struct GitTreeRead<'a> {
    pub input: &'a DependencyVerificationInput,
    pub repository: &'a str,
    pub subdirectory: Option<&'a str>,
    pub credential: Option<&'a str>,
    pub deadline: std::time::Instant,
}

impl std::fmt::Debug for GitTreeRead<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitTreeRead")
            .field("input", &self.input)
            .field("repository", &self.repository)
            .field("subdirectory", &self.subdirectory)
            .field("deadline", &self.deadline)
            .field("credential", &self.credential.map(|_| "<redacted>"))
            .finish()
    }
}

pub trait GitTreeReader: Send + Sync {
    fn read(&self, request: &GitTreeRead<'_>) -> Result<RevisionFiles>;
}

#[derive(Debug, Clone, Copy)]
pub struct NativeGitTreeReader;

impl GitTreeReader for NativeGitTreeReader {
    fn read(&self, request: &GitTreeRead<'_>) -> Result<RevisionFiles> {
        import_source(request, None)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GitSourceBinding<'a> {
    pub resource: &'a ManagedResourceId,
    pub source: &'a ManagedSourceId,
    pub relative_root: &'a str,
}

mod certificate_authority;

impl ManagedRepository {
    pub(super) async fn retain_git_verification(
        &self,
        owner: &UserId,
        manifest: &DependencyVerificationManifest,
    ) -> Result<DependencyVerificationManifest> {
        let root = manifest
            .revisions
            .iter()
            .find(|entry| entry.provenance.revision_id == manifest.root_revision_id)
            .ok_or(ManagedError::Integrity)?;
        let encoded = serde_json::to_value(manifest)?;
        sqlx::query!("INSERT INTO managed_dependency_verifications(id,owner_id,revision_id,commit_sha,bundle_digest,manifest) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(owner_id,revision_id,commit_sha,bundle_digest) DO NOTHING", manifest.id.as_str(), owner.as_str(), manifest.root_revision_id.as_str(), &root.provenance.exact_commit, manifest.bundle_digest.as_str(), encoded).execute(&self.pool).await?;
        self.verified_git_manifest(
            owner,
            &manifest.root_revision_id,
            &root.provenance.exact_commit,
        )
        .await
    }

    pub(super) async fn require_git_verification_binding(
        &self,
        owner: &UserId,
        capture: &DependencyVerificationInput,
        root: &ResourceRevisionId,
        retained_manifest: &crate::managed::RevisionManifest,
    ) -> Result<()> {
        let binding = sqlx::query!("SELECT r.source_id,r.resource_id,s.upstream_key FROM managed_revisions r JOIN managed_resources s ON s.owner_id=r.owner_id AND s.id=r.resource_id WHERE r.owner_id=$1 AND r.id=$2", owner.as_str(), capture.revision_id.as_str()).fetch_optional(&self.pool).await?.ok_or(ManagedError::Unavailable)?;
        let provenance = self
            .snapshot_provenance(owner, &retained_manifest.snapshot_id)
            .await?;
        let matches_git = provenance.source_kind == "git"
            && binding.source_id == capture.source_id.as_str()
            && binding.upstream_key == capture.relative_root
            && provenance.commit.as_deref() == Some(capture.exact_commit.as_str());
        let authored_root = capture.revision_id == *root
            && matches!(provenance.source_kind.as_str(), "local_tree" | "managed")
            && provenance.commit.is_none();
        let explicit_binding = if authored_root {
            sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM managed_resource_git_bindings WHERE owner_id=$1 AND resource_id=$2 AND source_id=$3 AND relative_root=$4)", owner.as_str(), &binding.resource_id, capture.source_id.as_str(), &capture.relative_root).fetch_one(&self.pool).await?.unwrap_or(false)
        } else {
            false
        };
        if !matches_git && !explicit_binding {
            return Err(ManagedError::Conflict("Verification requires matching immutable Git provenance or an explicit authored-root source binding".to_owned()));
        }
        Ok(())
    }

    pub async fn verify_git_content(
        &self,
        owner: &UserId,
        input: &GitContentVerification,
    ) -> Result<AssetDigest> {
        let request = DependencyVerificationRequest {
            root_revision_id: input.revision_id.clone(),
            revisions: vec![DependencyVerificationInput {
                revision_id: input.revision_id.clone(),
                source_id: input.source_id.clone(),
                relative_root: input.upstream_root.clone(),
                exact_commit: input.commit.clone(),
                dependencies: Vec::new(),
            }],
        };
        let manifest = self
            .verify_git_dependencies(owner, &request, &BTreeMap::new())
            .await?;
        Ok(AssetDigest::try_from(
            manifest.bundle_digest.as_str().to_owned(),
        )?)
    }

    pub async fn verify_git_dependencies(
        &self,
        owner: &UserId,
        input: &DependencyVerificationRequest,
        credentials: &BTreeMap<ManagedSourceId, String>,
    ) -> Result<DependencyVerificationManifest> {
        GitVerificationService::new(self.clone(), std::sync::Arc::new(NativeGitTreeReader))
            .verify(owner, input, credentials)
            .await
    }

    pub async fn bind_git_verification_source(
        &self,
        owner: &UserId,
        actor: &UserId,
        binding: &GitSourceBinding<'_>,
    ) -> Result<()> {
        let GitSourceBinding {
            resource,
            source,
            relative_root,
        } = *binding;
        systemprompt_models::managed::validate_path(relative_root)?;
        if !matches!(
            self.get_source(owner, source).await?,
            crate::managed::SourceSpec::Git { .. }
        ) {
            return Err(crate::managed::error::invalid(
                "Verification binding requires a Git source",
            ));
        }
        sqlx::query!("INSERT INTO managed_resource_git_bindings(owner_id,resource_id,source_id,relative_root,bound_by) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING", owner.as_str(), resource.as_str(), source.as_str(), relative_root, actor.as_str()).execute(&self.pool).await?;
        let matched = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM managed_resource_git_bindings WHERE owner_id=$1 AND resource_id=$2 AND source_id=$3 AND relative_root=$4)", owner.as_str(), resource.as_str(), source.as_str(), relative_root).fetch_one(&self.pool).await?.unwrap_or(false);
        if !matched {
            return Err(ManagedError::Conflict(
                "Resource already has a different retained Git source binding".to_owned(),
            ));
        }
        Ok(())
    }

    pub async fn git_verification(
        &self,
        owner: &UserId,
        id: &systemprompt_identifiers::DependencyVerificationId,
    ) -> Result<DependencyVerificationManifest> {
        let encoded = sqlx::query_scalar!(
            "SELECT manifest FROM managed_dependency_verifications WHERE owner_id=$1 AND id=$2",
            owner.as_str(),
            id.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ManagedError::Unavailable)?;
        let manifest: DependencyVerificationManifest = serde_json::from_value(encoded)?;
        manifest
            .validate_complete()
            .map_err(|_error| ManagedError::Integrity)?;
        if manifest.id != *id {
            return Err(ManagedError::Integrity);
        }
        Ok(manifest)
    }

    pub async fn verified_git_manifest(
        &self,
        owner: &UserId,
        revision: &ResourceRevisionId,
        commit: &str,
    ) -> Result<DependencyVerificationManifest> {
        let digest = self.get_revision_bundle(owner, revision).await?.digest()?;
        let encoded = sqlx::query_scalar!("SELECT manifest FROM managed_dependency_verifications WHERE owner_id=$1 AND revision_id=$2 AND commit_sha=$3 AND bundle_digest=$4", owner.as_str(), revision.as_str(), commit, digest.as_str()).fetch_optional(&self.pool).await?.ok_or_else(|| ManagedError::Conflict("A complete retained server-side Git dependency verification is required".to_owned()))?;
        let manifest: DependencyVerificationManifest = serde_json::from_value(encoded)?;
        manifest
            .validate_complete()
            .map_err(|_error| ManagedError::Integrity)?;
        if manifest.root_revision_id != *revision
            || manifest.bundle_digest.as_str() != digest.as_str()
        {
            return Err(ManagedError::Integrity);
        }
        Ok(manifest)
    }

    pub async fn require_verified_git_content(
        &self,
        owner: &UserId,
        revision: &ResourceRevisionId,
        commit: &str,
    ) -> Result<()> {
        self.verified_git_manifest(owner, revision, commit).await?;
        Ok(())
    }
}

pub(super) fn require_matching_files(
    files: &RevisionFiles,
    retained: &RevisionFiles,
) -> Result<()> {
    if files.0.is_empty()
        || files.0.len() != retained.0.len()
        || files.0.iter().any(|(path, file)| {
            retained.0.get(path).is_none_or(|expected| {
                expected.bytes != file.bytes || expected.executable != file.executable
            })
        })
    {
        return Err(ManagedError::Conflict("Git bytes or modes differ from the retained revision; import and reevaluate changed content".to_owned()));
    }
    Ok(())
}

fn import_source(
    request: &GitTreeRead<'_>,
    certificate_authority: Option<&[u8]>,
) -> Result<RevisionFiles> {
    let GitTreeRead {
        input,
        repository,
        subdirectory,
        credential,
        deadline,
    } = *request;
    let temp = std::env::temp_dir().join(format!(
        "systemprompt-verification-{}",
        ManagedSourceId::generate()
    ));
    crate::managed::git_execution::create_private_directory(&temp)?;
    let imported = import_tree(&GitCheckout {
        temp: &temp,
        repository,
        commit: &input.exact_commit,
        subdirectory,
        root: &input.relative_root,
        credential,
        certificate_authority,
        deadline,
    });
    std::fs::remove_dir_all(&temp)?;
    if temp.exists() {
        return Err(ManagedError::Integrity);
    }
    imported
}
