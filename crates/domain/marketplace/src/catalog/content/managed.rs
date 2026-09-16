//! Managed catalog projections preserve organization ownership and grants.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::CatalogContent;
use crate::error::MarketplaceError;
use crate::managed::{
    ManagedRepository, ManagedSkillResolution, OrganizationSkillResolver, ResourceKind,
};
use systemprompt_identifiers::UserId;

impl CatalogContent {
    pub async fn with_managed_skills(
        mut self,
        repository: ManagedRepository,
        owner: &UserId,
    ) -> Result<Self, MarketplaceError> {
        let resolver = crate::managed::ManagedResourceResolver::new(repository.clone());
        let mut offset = 0;
        loop {
            let page = repository
                .list_resources(owner, offset)
                .await
                .map_err(MarketplaceError::Managed)?;
            let has_more = page.has_more;
            for resource in page
                .items
                .into_iter()
                .filter(|item| item.kind == ResourceKind::Skill)
            {
                let resolution = resolver
                    .resolve_skill(owner, &resource.resource_key)
                    .await
                    .map_err(MarketplaceError::Managed)?;
                self.remove_managed_key(&resource.resource_key);
                match resolution {
                    ManagedSkillResolution::Published(skill) => {
                        let (entry, files) =
                            crate::catalog::skills::build_managed_skill_entry(*skill)?;
                        self.managed_files.insert(entry.id.clone(), files);
                        self.skills.push(entry);
                    },
                    ManagedSkillResolution::Withheld(reason) => {
                        tracing::info!(
                            skill = %resource.resource_key,
                            reason = reason.as_str(),
                            "managed skill withheld from the catalogue"
                        );
                    },
                    ManagedSkillResolution::NotManaged => {
                        return Err(MarketplaceError::Catalog(format!(
                            "managed skill {} lost its resource binding",
                            resource.resource_key
                        )));
                    },
                }
            }
            if !has_more {
                break;
            }
            offset += ManagedRepository::PAGE_SIZE;
        }
        self.skills
            .sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        Ok(self)
    }

    pub async fn with_organization_skills(
        self,
        repository: ManagedRepository,
        owner: &UserId,
        consumer: &UserId,
    ) -> Result<Self, MarketplaceError> {
        let mut catalog = self
            .with_managed_skills(repository.clone(), consumer)
            .await?;
        let resolver = OrganizationSkillResolver::new(repository.clone(), owner.clone());
        let mut offset = 0;
        loop {
            let page = repository
                .list_resources(owner, offset)
                .await
                .map_err(MarketplaceError::Managed)?;
            let has_more = page.has_more;
            for resource in page
                .items
                .into_iter()
                .filter(|resource| resource.kind == ResourceKind::Skill)
            {
                catalog.remove_managed_key(&resource.resource_key);
                if let ManagedSkillResolution::Published(skill) = resolver
                    .resolve_skill_for_catalog(consumer, &resource.resource_key)
                    .await
                    .map_err(MarketplaceError::Managed)?
                {
                    let (entry, files) = crate::catalog::skills::build_managed_skill_entry(*skill)?;
                    catalog.managed_files.insert(entry.id.clone(), files);
                    catalog.skills.push(entry);
                }
            }
            if !has_more {
                break;
            }
            offset += ManagedRepository::PAGE_SIZE;
        }
        catalog
            .skills
            .sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        Ok(catalog)
    }

    pub async fn without_organization_skills(
        mut self,
        repository: ManagedRepository,
        owner: &UserId,
    ) -> Result<Self, MarketplaceError> {
        let mut offset = 0;
        loop {
            let page = repository
                .list_resources(owner, offset)
                .await
                .map_err(MarketplaceError::Managed)?;
            let has_more = page.has_more;
            for resource in page
                .items
                .into_iter()
                .filter(|resource| resource.kind == ResourceKind::Skill)
            {
                self.remove_managed_key(&resource.resource_key);
            }
            if !has_more {
                break;
            }
            offset += ManagedRepository::PAGE_SIZE;
        }
        Ok(self)
    }

    fn remove_managed_key(&mut self, key: &str) {
        self.skills.retain(|entry| entry.id.as_str() != key);
        self.managed_files.retain(|id, _| id.as_str() != key);
    }
}
