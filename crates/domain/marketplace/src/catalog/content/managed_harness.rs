//! Managed plugins and rules in the catalogue: their publication identity.
//!
//! A plugin or rule whose key is a managed resource of that kind is withheld
//! unless the consumer resolves it (cohorts included) to a published
//! generation; a published one carries its [`ManagedPublicationRef`] into the
//! signed manifest so installation receipts cover the whole harness, not only
//! skills. An unmanaged key is untouched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::{PluginId, UserId};
use systemprompt_models::bridge::ids::Sha256Digest;
use systemprompt_models::bridge::manifest::{ManagedPublicationRef, PluginEntry};

use super::CatalogContent;
use super::managed::{admit_published, revoked_keys};
use crate::error::MarketplaceError;
use crate::managed::{ManagedRepository, ManagedResolution, ResourceKind};

fn publication_ref(
    resolution: &ManagedResolution,
) -> Result<Option<ManagedPublicationRef>, MarketplaceError> {
    let ManagedResolution::Published {
        publication_id,
        resource_id,
        generation,
        revision_id,
        bundle_digest,
    } = resolution
    else {
        return Ok(None);
    };
    Ok(Some(ManagedPublicationRef {
        publication_id: publication_id.clone(),
        resource_id: resource_id.clone(),
        revision_id: revision_id.clone(),
        generation: *generation,
        bundle_digest: Sha256Digest::try_new(bundle_digest.as_str())
            .map_err(|error| MarketplaceError::catalog("managed bundle digest", error))?,
    }))
}

impl CatalogContent {
    pub(super) async fn overlay_harness(
        &mut self,
        repository: &ManagedRepository,
        owner: &UserId,
        consumer: &UserId,
    ) -> Result<(), MarketplaceError> {
        for kind in [ResourceKind::Plugin, ResourceKind::Rule] {
            let revoked = revoked_keys(repository, owner, consumer, kind).await?;
            let rows = repository
                .list_resolutions_for(owner, consumer, kind)
                .await
                .map_err(MarketplaceError::Managed)?;
            let admitted = admit_published(rows, &revoked, kind)?;
            let mut published = BTreeMap::new();
            for (key, resolution) in &admitted.published {
                if let Some(reference) = publication_ref(resolution)? {
                    published.insert(key.clone(), reference);
                }
            }
            match kind {
                ResourceKind::Rule => {
                    self.rules.retain_mut(|rule| {
                        let key = rule.id.as_str();
                        if let Some(reference) = published.get(key) {
                            rule.publication = Some(reference.clone());
                            return true;
                        }
                        !admitted.managed.contains(key)
                    });
                },
                _ => {
                    for key in &admitted.managed {
                        let id = PluginId::try_new(key.as_str()).map_err(|error| {
                            MarketplaceError::catalog("managed plugin id", error)
                        })?;
                        match published.remove(key) {
                            Some(reference) => {
                                self.plugin_publications.insert(id, reference);
                            },
                            None => {
                                self.withheld_plugins.insert(id);
                            },
                        }
                    }
                },
            }
        }
        Ok(())
    }

    pub(super) async fn withhold_harness(
        &mut self,
        repository: &ManagedRepository,
        owner: &UserId,
    ) -> Result<(), MarketplaceError> {
        let rules = repository
            .list_resolutions(owner, ResourceKind::Rule)
            .await
            .map_err(MarketplaceError::Managed)?;
        self.rules
            .retain(|rule| !rules.iter().any(|row| row.resource_key == rule.id.as_str()));
        for row in repository
            .list_resolutions(owner, ResourceKind::Plugin)
            .await
            .map_err(MarketplaceError::Managed)?
        {
            let id = PluginId::try_new(row.resource_key.as_str())
                .map_err(|error| MarketplaceError::catalog("managed plugin id", error))?;
            self.withheld_plugins.insert(id);
        }
        Ok(())
    }

    #[must_use]
    pub fn stamp_plugins(&self, plugins: Vec<PluginEntry>) -> Vec<PluginEntry> {
        plugins
            .into_iter()
            .filter(|plugin| !self.withheld_plugins.contains(&plugin.id))
            .map(|mut plugin| {
                plugin.publication = self.plugin_publications.get(&plugin.id).cloned();
                plugin
            })
            .collect()
    }
}
