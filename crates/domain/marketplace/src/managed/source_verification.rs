//! Prove Git content independently of caller-asserted snapshot provenance.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::git_import::{GitCheckout, import_tree};
use super::git_spec;
use crate::managed::{AssetDigest, ManagedError, ManagedRepository, Result};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ManagedSourceId, ResourceRevisionId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitContentVerification {
    pub revision_id: ResourceRevisionId,
    pub source_id: ManagedSourceId,
    pub upstream_root: String,
    pub commit: String,
}

impl ManagedRepository {
    pub async fn verify_git_content(
        &self,
        owner: &UserId,
        input: &GitContentVerification,
    ) -> Result<AssetDigest> {
        systemprompt_models::managed::validate_path(&input.upstream_root)?;
        if !matches!(input.commit.len(), 40 | 64)
            || !input
                .commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(crate::managed::error::invalid(
                "Verification requires an exact Git commit",
            ));
        }
        let (repository, _, subdirectory) =
            git_spec(self.get_source(owner, &input.source_id).await?)?;
        systemprompt_models::net::validate_outbound_url(&repository)
            .map_err(|error| crate::managed::error::invalid(&error.to_string()))?;
        let bundle = self.get_revision_bundle(owner, &input.revision_id).await?;
        if bundle.revisions.len() != 1 {
            return Err(crate::managed::error::invalid(
                "Git verification of dependency bundles requires separate dependency provenance; single-resource verification cannot attest them",
            ));
        }
        let capture = input.clone();
        let files = tokio::task::spawn_blocking(move || {
            import_source(&capture, &repository, subdirectory.as_deref())
        })
        .await
        .map_err(|error| ManagedError::Io(std::io::Error::other(error)))??;
        let retained = bundle.revision_files(&bundle.root)?;
        if files.0.len() != retained.0.len()
            || files.0.iter().any(|(path, file)| {
                retained.0.get(path).is_none_or(|expected| {
                    expected.bytes != file.bytes || expected.executable != file.executable
                })
            })
        {
            return Err(ManagedError::Conflict("Git commit content differs from the retained revision; import and reevaluate the changed content".to_owned()));
        }
        let digest = bundle.digest()?;
        sqlx::query!("INSERT INTO managed_git_verifications(owner_id,revision_id,commit_sha,source_id,upstream_root,bundle_digest) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING", owner.as_str(), input.revision_id.as_str(), &input.commit, input.source_id.as_str(), &input.upstream_root, digest.as_str()).execute(&self.pool).await?;
        Ok(digest)
    }

    pub async fn require_verified_git_content(
        &self,
        owner: &UserId,
        revision: &ResourceRevisionId,
        commit: &str,
    ) -> Result<()> {
        let digest = self.get_revision_bundle(owner, revision).await?.digest()?;
        let verified = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM managed_git_verifications WHERE owner_id=$1 AND revision_id=$2 AND commit_sha=$3 AND bundle_digest=$4)", owner.as_str(), revision.as_str(), commit, digest.as_str()).fetch_one(&self.pool).await?.unwrap_or(false);
        if !verified {
            return Err(ManagedError::Conflict(
                "A retained server-side Git content verification is required".to_owned(),
            ));
        }
        Ok(())
    }
}

fn import_source(
    input: &GitContentVerification,
    repository: &str,
    subdirectory: Option<&str>,
) -> Result<crate::managed::RevisionFiles> {
    let temp = std::env::temp_dir().join(format!(
        "systemprompt-verification-{}",
        ManagedSourceId::generate()
    ));
    std::fs::create_dir(&temp)?;
    let imported = import_tree(&GitCheckout {
        temp: &temp,
        repository,
        commit: &input.commit,
        subdirectory,
        root: &input.upstream_root,
        credential: None,
    });
    let cleanup = std::fs::remove_dir_all(&temp);
    let files = imported?;
    cleanup?;
    Ok(files)
}
