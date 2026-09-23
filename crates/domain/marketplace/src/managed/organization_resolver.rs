//! Explicit organization authority with consumer-scoped grants.
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

impl ManagedRepository {
    async fn has_active_skill_grant(
        &self,
        owner: &UserId,
        consumer: &UserId,
        key: &str,
    ) -> Result<bool> {
        Ok(sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM managed_consumer_grants g JOIN managed_resources r ON r.id=g.resource_id AND r.owner_id=g.owner_id WHERE g.owner_id=$1 AND g.consumer_id=$2 AND r.kind='skill' AND r.resource_key=$3 AND g.revoked_at IS NULL) AS \"granted!\"", owner.as_str(), consumer.as_str(), key).fetch_one(&self.pool).await?)
    }
}
