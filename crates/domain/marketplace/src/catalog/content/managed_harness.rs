//! Managed plugins and rules in the catalogue: their publication identity.
//!
//! A plugin or rule whose key is a managed resource of that kind is withheld
//! unless the consumer resolves it (cohorts included) to a published
//! generation. A published one is served from its verified revision — a rule's
//! `config.yaml` and content file, a plugin's own files (scripts, Node package
//! files, `hosts/`) — and carries its [`ManagedPublicationRef`] into the
//! signed manifest, so an installation receipt names exactly what ran. An
//! unmanaged key is untouched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use systemprompt_identifiers::{PluginId, UserId};
use systemprompt_models::bridge::ids::Sha256Digest;
use systemprompt_models::bridge::manifest::{ManagedPublicationRef, PluginEntry};

use super::CatalogContent;
use super::managed::{admit_published, revoked_keys, verified_bundles};
use crate::error::MarketplaceError;
use crate::managed::{
    ManagedError, ManagedRepository, ManagedResolution, ResourceKind, RevisionBundle,
};
use systemprompt_manifest::services::DiskRuleConfig;

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
            let bundles = if admitted.published.is_empty() {
                BTreeMap::new()
            } else {
                verified_bundles(repository, owner, &admitted.published).await?
            };
            let mut published = BTreeMap::new();
            for (key, resolution) in &admitted.published {
                let (Some(reference), ManagedResolution::Published { revision_id, .. }) =
                    (publication_ref(resolution)?, resolution)
                else {
                    continue;
                };
                let bundle = bundles
                    .get(revision_id)
                    .ok_or(MarketplaceError::Managed(ManagedError::Integrity))?;
                published.insert(key.clone(), (reference, Arc::clone(bundle)));
            }
            match kind {
                ResourceKind::Rule => self.overlay_rules(&admitted.managed, published)?,
                _ => self.overlay_plugins(&admitted.managed, published)?,
            }
        }
        Ok(())
    }

    fn overlay_rules(
        &mut self,
        managed: &BTreeSet<String>,
        published: BTreeMap<String, (ManagedPublicationRef, Arc<RevisionBundle>)>,
    ) -> Result<(), MarketplaceError> {
        self.rules
            .retain(|rule| !managed.contains(rule.id.as_str()));
        for (key, (reference, bundle)) in published {
            let mut entry = managed_rule(&key, &reference, &bundle)?;
            entry.publication = Some(reference);
            self.rules.push(entry);
        }
        self.rules.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        Ok(())
    }

    fn overlay_plugins(
        &mut self,
        managed: &BTreeSet<String>,
        mut published: BTreeMap<String, (ManagedPublicationRef, Arc<RevisionBundle>)>,
    ) -> Result<(), MarketplaceError> {
        for key in managed {
            let id = PluginId::try_new(key.as_str())
                .map_err(|error| MarketplaceError::catalog("managed plugin id", error))?;
            match published.remove(key) {
                Some((reference, bundle)) => {
                    let files = bundle
                        .revision_files(&bundle.root)
                        .map_err(|e| MarketplaceError::Managed(ManagedError::from(e)))?;
                    self.managed_plugin_files.insert(id.clone(), files);
                    self.plugin_publications.insert(id, reference);
                },
                None => {
                    self.withheld_plugins.insert(id);
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

fn managed_rule(
    key: &str,
    reference: &ManagedPublicationRef,
    bundle: &RevisionBundle,
) -> Result<systemprompt_models::bridge::manifest::RuleEntry, MarketplaceError> {
    let integrity = || MarketplaceError::Managed(ManagedError::Integrity);
    let files = bundle
        .revision_files(&bundle.root)
        .map_err(|e| MarketplaceError::Managed(ManagedError::from(e)))?;
    let config_file = files.0.get("config.yaml").ok_or_else(integrity)?;
    let config: DiskRuleConfig =
        serde_yaml::from_slice(&config_file.bytes).map_err(|_corrupt| integrity())?;
    if !config.enabled {
        return Err(integrity());
    }
    let content = files.0.get(config.content_file()).ok_or_else(integrity)?;
    let raw = std::str::from_utf8(&content.bytes).map_err(|_corrupt| integrity())?;
    crate::catalog::rule_entry(
        key,
        config,
        raw,
        format!("managed://{key}@{}", reference.bundle_digest.as_str()),
    )
}
