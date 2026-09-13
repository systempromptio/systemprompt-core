//! Fail-closed managed resolution shared by every runtime consumer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{SkillId, UserId};
use systemprompt_models::{DiskSkillConfig, strip_frontmatter};

use super::{
    ManagedError, ManagedRepository, ManagedResolution, ResourceKind, Result, RevisionBundle,
};

#[derive(Debug, Clone)]
pub enum ResolvedManagedResource {
    NotManaged,
    NeverAdopted(ManagedResolution),
    Published {
        state: ManagedResolution,
        bundle: RevisionBundle,
    },
    Withdrawn(ManagedResolution),
    IntegrityFailure(ManagedResolution),
}

#[derive(Debug, Clone)]
pub struct ManagedResourceResolver {
    repository: ManagedRepository,
}

#[derive(Debug, Clone)]
pub struct ManagedSkill {
    pub id: SkillId,
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub files: super::RevisionFiles,
    pub generation: i64,
    pub bundle_digest: super::AssetDigest,
}

impl ManagedResourceResolver {
    pub const fn new(repository: ManagedRepository) -> Self {
        Self { repository }
    }

    pub async fn resolve(
        &self,
        owner: &UserId,
        kind: ResourceKind,
        resource_key: &str,
    ) -> Result<ResolvedManagedResource> {
        let state = self
            .repository
            .resolve_managed(owner, kind, resource_key)
            .await?;
        match &state {
            ManagedResolution::NotManaged => Ok(ResolvedManagedResource::NotManaged),
            ManagedResolution::NeverAdopted { .. } => {
                Ok(ResolvedManagedResource::NeverAdopted(state))
            },
            ManagedResolution::Withdrawn { .. } => Ok(ResolvedManagedResource::Withdrawn(state)),
            ManagedResolution::IntegrityFailure { .. } => {
                Ok(ResolvedManagedResource::IntegrityFailure(state))
            },
            ManagedResolution::Published {
                resource_id,
                generation,
                bundle_digest,
                ..
            } => {
                match self
                    .repository
                    .get_publication_bundle(owner, resource_id, *generation, bundle_digest)
                    .await
                {
                    Ok(bundle) => Ok(ResolvedManagedResource::Published { state, bundle }),
                    Err(ManagedError::Integrity | ManagedError::Unavailable) => {
                        Ok(ResolvedManagedResource::IntegrityFailure(
                            ManagedResolution::IntegrityFailure {
                                resource_id: resource_id.clone(),
                                generation: *generation,
                            },
                        ))
                    },
                    Err(error) => Err(error),
                }
            },
        }
    }

    pub async fn resolve_state(
        &self,
        owner: &UserId,
        kind: ResourceKind,
        key: &str,
    ) -> Result<ManagedResolution> {
        Ok(match self.resolve(owner, kind, key).await? {
            ResolvedManagedResource::NotManaged => ManagedResolution::NotManaged,
            ResolvedManagedResource::NeverAdopted(state)
            | ResolvedManagedResource::Withdrawn(state)
            | ResolvedManagedResource::IntegrityFailure(state)
            | ResolvedManagedResource::Published { state, .. } => state,
        })
    }

    /// Resolve a runtime skill. `Ok(None)` is returned only when the key has
    /// never entered managed-resource ownership; every managed non-published
    /// or corrupt state fails closed.
    pub async fn resolve_skill(&self, owner: &UserId, key: &str) -> Result<Option<ManagedSkill>> {
        match self.resolve(owner, ResourceKind::Skill, key).await? {
            ResolvedManagedResource::NotManaged => Ok(None),
            ResolvedManagedResource::Published { state, bundle } => {
                let ManagedResolution::Published {
                    generation,
                    bundle_digest,
                    ..
                } = state
                else {
                    return Err(ManagedError::Integrity);
                };
                let files = bundle.revision_files(&bundle.root)?;
                let config_file = files.0.get("config.yaml").ok_or(ManagedError::Integrity)?;
                let config: DiskSkillConfig = serde_yaml::from_slice(&config_file.bytes)
                    .map_err(|_| ManagedError::Integrity)?;
                if !config.enabled || (!config.id.as_str().is_empty() && config.id.as_str() != key)
                {
                    return Err(ManagedError::Integrity);
                }
                let content = files
                    .0
                    .get(config.content_file())
                    .ok_or(ManagedError::Integrity)?;
                let raw =
                    std::str::from_utf8(&content.bytes).map_err(|_| ManagedError::Integrity)?;
                Ok(Some(ManagedSkill {
                    id: if config.id.as_str().is_empty() {
                        SkillId::new(key.to_owned())
                    } else {
                        config.id
                    },
                    name: if config.name.is_empty() {
                        key.to_owned()
                    } else {
                        config.name
                    },
                    description: config.description,
                    instructions: strip_frontmatter(raw),
                    files,
                    generation,
                    bundle_digest,
                }))
            },
            ResolvedManagedResource::NeverAdopted(_)
            | ResolvedManagedResource::Withdrawn(_)
            | ResolvedManagedResource::IntegrityFailure(_) => Err(ManagedError::Integrity),
        }
    }
}
