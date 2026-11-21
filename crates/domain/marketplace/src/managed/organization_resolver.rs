//! Explicit organization authority with consumer-scoped grants.
//!
//! [`OrganizationSkillResolver`] is bound to one owner: a consumer's skill
//! resolves through the owner's grants, and
//! [`OrganizationSkillResolver::owned_skills`]
//! and [`OrganizationSkillResolver::resolve_owned`] read the owner's own
//! managed skills without handing the owner id to the caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;
use systemprompt_traits::{
    ManagedSkillResolver, ManagedSkillResolverError, SkillResolution, WithheldReason,
};

use super::{
    ManagedRepository, ManagedResolution, ManagedResourceResolver, ManagedSkillResolution,
    ResourceKind, Result,
};

/// One managed skill key of the owner's inventory and how it resolves.
#[derive(Debug)]
pub struct OwnedSkill {
    pub key: String,
    pub resolution: ManagedSkillResolution,
}

#[derive(Debug, Clone)]
pub struct OrganizationSkillResolver {
    resolver: ManagedResourceResolver,
    owner: UserId,
}

impl OrganizationSkillResolver {
    pub const fn new(repository: ManagedRepository, owner: UserId) -> Self {
        Self {
            resolver: ManagedResourceResolver::new(repository),
            owner,
        }
    }

    pub async fn resolve_skill(
        &self,
        consumer: &UserId,
        key: &str,
    ) -> Result<ManagedSkillResolution> {
        let repository = self.resolver.repository();
        let state = repository
            .resolve_managed(&self.owner, ResourceKind::Skill, key)
            .await?;
        if matches!(state, ManagedResolution::NotManaged) {
            return self.resolver.resolve_skill(consumer, key).await;
        }
        if !self.granted(consumer, key).await? {
            return Ok(ManagedSkillResolution::Withheld(Box::new(
                WithheldReason::NotGranted,
            )));
        }
        let resolved = self.resolver.resolve_skill(&self.owner, key).await?;
        if !self.granted(consumer, key).await? {
            return Ok(ManagedSkillResolution::Withheld(Box::new(
                WithheldReason::NotGranted,
            )));
        }
        Ok(resolved)
    }

    pub async fn resolve_owned(&self, key: &str) -> Result<ManagedSkillResolution> {
        self.resolver.resolve_skill(&self.owner, key).await
    }

    pub async fn owned_skills(&self) -> Result<Vec<OwnedSkill>> {
        let repository = self.resolver.repository();
        let mut owned = Vec::new();
        let mut offset = 0;
        loop {
            let page = repository.list_resources(&self.owner, offset).await?;
            for resource in page
                .items
                .into_iter()
                .filter(|resource| resource.kind == ResourceKind::Skill)
            {
                let resolution = self.resolve_owned(&resource.resource_key).await?;
                owned.push(OwnedSkill {
                    key: resource.resource_key,
                    resolution,
                });
            }
            if !page.has_more {
                return Ok(owned);
            }
            offset += ManagedRepository::PAGE_SIZE;
        }
    }

    async fn granted(&self, consumer: &UserId, key: &str) -> Result<bool> {
        if consumer == &self.owner {
            return Ok(true);
        }
        self.resolver
            .repository()
            .has_active_skill_grant(&self.owner, consumer, key)
            .await
    }
}

#[async_trait::async_trait]
impl ManagedSkillResolver for OrganizationSkillResolver {
    async fn resolve_skill(
        &self,
        consumer: &UserId,
        key: &str,
    ) -> std::result::Result<SkillResolution, ManagedSkillResolverError> {
        super::resolver::runtime_resolution(Self::resolve_skill(self, consumer, key).await, key)
    }
}
