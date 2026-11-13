//! Read accessors over the `ExtensionRegistry`.
//!
//! [`ExtensionRegistry::enabled_extensions`] is the one definition of
//! "enabled". Every collector of extension contributions (schemas, routers,
//! site auth, config, assets, storage paths, config planes, frontmatter
//! processors, metrics, diagnostics,
//! readiness checks) takes the profile's disabled set and walks only the
//! enabled extensions, so a disabled extension contributes nothing. `get`,
//! `has`, `ids` and `extensions` describe the linked inventory and are not
//! collectors.
//!
//! Schema installation and migrations are the one deliberate exception, and
//! [`ExtensionRegistry::INSTALL_EVERY_LINKED_SCHEMA`] is where it is decided:
//! the boot install, the schema-currency check and `infra db migrate` create
//! and migrate the tables of every linked extension, disabled or not. Linked
//! code reads and writes those tables whatever the profile disables (the user
//! purge inventory, the kernel-composed repositories, cross-extension views
//! and triggers), and re-enabling an extension must find its tables current.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ExtensionRegistry;
use crate::Extension;
use crate::asset::{AssetDefinition, AssetPaths};
use crate::context::ExtensionContext;
use crate::error::LoaderError;
use crate::health::{DiagnosticProbe, MetricDescriptor, ReadinessCheck};
use std::sync::Arc;
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{ConfigPlane, FrontmatterProcessor};

pub type ApiRouter = (Arc<dyn Extension>, crate::ExtensionRouter);

pub type ExtensionReadinessCheck = (&'static str, Arc<dyn ReadinessCheck>);

pub type ExtensionDiagnostic = (&'static str, Arc<dyn DiagnosticProbe>);

impl ExtensionRegistry {
    pub const INSTALL_EVERY_LINKED_SCHEMA: &'static [ExtensionId] = &[];

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Arc<dyn Extension>> {
        self.extensions.get(id)
    }

    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.extensions.contains_key(id)
    }

    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.extensions.keys().map(String::as_str).collect()
    }

    #[must_use]
    pub fn extensions(&self) -> &[Arc<dyn Extension>] {
        &self.sorted_extensions
    }

    // Why: a disabled set is a policy over the loaded inventory; disabling a
    // dependency of an enabled extension would leave that extension booting
    // against tables its dependency never created, so the whole set is
    // refused rather than partially applied.
    pub fn enabled_extensions(
        &self,
        disabled_ids: &[ExtensionId],
    ) -> Result<Vec<Arc<dyn Extension>>, LoaderError> {
        let is_disabled = |id: &str| disabled_ids.iter().any(|d| d == id);
        for ext in &self.sorted_extensions {
            if ext.is_required() && is_disabled(ext.id()) {
                return Err(LoaderError::RequiredExtensionDisabled(ExtensionId::new(
                    ext.id(),
                )));
            }
        }
        for ext in &self.sorted_extensions {
            if is_disabled(ext.id()) {
                continue;
            }
            if let Some(dep) = ext.dependencies().into_iter().find(|d| is_disabled(d)) {
                return Err(LoaderError::DisabledDependency {
                    extension: ExtensionId::new(ext.id()),
                    dependency: ExtensionId::new(dep),
                });
            }
        }
        Ok(self
            .sorted_extensions
            .iter()
            .filter(|ext| !is_disabled(ext.id()))
            .cloned()
            .collect())
    }

    fn enabled_flat_map<T, I>(
        &self,
        disabled_ids: &[ExtensionId],
        contribute: impl Fn(&Arc<dyn Extension>) -> I,
    ) -> Result<Vec<T>, LoaderError>
    where
        I: IntoIterator<Item = T>,
    {
        Ok(self
            .enabled_extensions(disabled_ids)?
            .iter()
            .flat_map(contribute)
            .collect())
    }

    fn enabled_where(
        &self,
        disabled_ids: &[ExtensionId],
        keep: impl Fn(&Arc<dyn Extension>) -> bool,
    ) -> Result<Vec<Arc<dyn Extension>>, LoaderError> {
        Ok(self
            .enabled_extensions(disabled_ids)?
            .into_iter()
            .filter(|ext| keep(ext))
            .collect())
    }

    pub fn enabled_schema_extensions(
        &self,
        disabled_ids: &[ExtensionId],
    ) -> Result<Vec<Arc<dyn Extension>>, LoaderError> {
        self.enabled_where(disabled_ids, |e| e.has_schemas() || e.has_migrations())
    }

    pub fn api_routers(
        &self,
        ctx: &dyn ExtensionContext,
        disabled_ids: &[ExtensionId],
    ) -> Result<Vec<ApiRouter>, LoaderError> {
        Ok(self
            .enabled_extensions(disabled_ids)?
            .into_iter()
            .filter_map(|e| e.router(ctx).map(|r| (e, r)))
            .collect())
    }

    pub fn enabled_site_auth(
        &self,
        disabled_ids: &[ExtensionId],
    ) -> Result<Option<crate::SiteAuthConfig>, LoaderError> {
        Ok(self
            .enabled_extensions(disabled_ids)?
            .iter()
            .find_map(|e| e.site_auth()))
    }

    pub fn config_extensions(
        &self,
        disabled_ids: &[ExtensionId],
    ) -> Result<Vec<Arc<dyn Extension>>, LoaderError> {
        self.enabled_where(disabled_ids, |e| e.has_config())
    }

    pub fn asset_extensions(
        &self,
        disabled_ids: &[ExtensionId],
    ) -> Result<Vec<Arc<dyn Extension>>, LoaderError> {
        self.enabled_where(disabled_ids, |e| e.declares_assets())
    }

    pub fn all_required_storage_paths(
        &self,
        disabled_ids: &[ExtensionId],
    ) -> Result<Vec<&'static str>, LoaderError> {
        self.enabled_flat_map(disabled_ids, |e| e.required_storage_paths())
    }

    pub fn all_required_assets(
        &self,
        paths: &dyn AssetPaths,
        disabled_ids: &[ExtensionId],
    ) -> Result<Vec<(&'static str, AssetDefinition)>, LoaderError> {
        self.enabled_flat_map(disabled_ids, |e| {
            let id = e.id();
            e.required_assets(paths)
                .into_iter()
                .map(move |asset| (id, asset))
        })
    }

    pub fn all_config_planes(
        &self,
        disabled_ids: &[ExtensionId],
    ) -> Result<Vec<Arc<dyn ConfigPlane>>, LoaderError> {
        self.enabled_flat_map(disabled_ids, |e| e.config_planes())
    }

    pub fn frontmatter_processors(
        &self,
        disabled_ids: &[ExtensionId],
    ) -> Result<Vec<Arc<dyn FrontmatterProcessor>>, LoaderError> {
        self.enabled_flat_map(disabled_ids, |e| e.frontmatter_processors())
    }

    pub fn all_metrics(
        &self,
        disabled_ids: &[ExtensionId],
    ) -> Result<Vec<MetricDescriptor>, LoaderError> {
        self.enabled_flat_map(disabled_ids, |e| e.metrics())
    }

    pub fn all_readiness_checks(
        &self,
        ctx: &dyn ExtensionContext,
        disabled_ids: &[ExtensionId],
    ) -> Result<Vec<ExtensionReadinessCheck>, LoaderError> {
        self.enabled_flat_map(disabled_ids, |ext| {
            let id = ext.id();
            ext.readiness(ctx).into_iter().map(move |check| (id, check))
        })
    }

    pub fn all_diagnostics(
        &self,
        ctx: &dyn ExtensionContext,
        disabled_ids: &[ExtensionId],
    ) -> Result<Vec<ExtensionDiagnostic>, LoaderError> {
        self.enabled_flat_map(disabled_ids, |ext| {
            let id = ext.id();
            ext.diagnostics(ctx).into_iter().map(move |probe| (id, probe))
        })
    }
}
