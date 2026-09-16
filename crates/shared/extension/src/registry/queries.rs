//! Read accessors over the `ExtensionRegistry` for looking up extensions,
//! assets, and jobs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ExtensionRegistry;
use crate::Extension;
use crate::asset::{AssetDefinition, AssetPaths};
use crate::error::LoaderError;
use std::sync::Arc;
use systemprompt_provider_contracts::Job;

impl ExtensionRegistry {
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

    #[must_use]
    pub fn schema_extensions(&self) -> Vec<Arc<dyn Extension>> {
        self.sorted_extensions
            .iter()
            .filter(|e| e.has_schemas())
            .cloned()
            .collect()
    }

    // Why: a disabled set is a policy over the loaded inventory; disabling a
    // dependency of an enabled extension would leave that extension booting
    // against tables its dependency never created, so the whole set is
    // refused rather than partially applied.
    pub fn enabled_extensions(
        &self,
        disabled_ids: &[String],
    ) -> Result<Vec<Arc<dyn Extension>>, LoaderError> {
        let is_disabled = |id: &str| disabled_ids.iter().any(|d| d == id);
        for ext in &self.sorted_extensions {
            if ext.is_required() && is_disabled(ext.id()) {
                return Err(LoaderError::RequiredExtensionDisabled(ext.id().to_owned()));
            }
        }
        for ext in &self.sorted_extensions {
            if is_disabled(ext.id()) {
                continue;
            }
            if let Some(dep) = ext.dependencies().into_iter().find(|d| is_disabled(d)) {
                return Err(LoaderError::DisabledDependency {
                    extension: ext.id().to_owned(),
                    dependency: dep.to_owned(),
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

    pub fn enabled_schema_extensions(
        &self,
        disabled_ids: &[String],
    ) -> Result<Vec<Arc<dyn Extension>>, LoaderError> {
        Ok(self
            .enabled_extensions(disabled_ids)?
            .into_iter()
            .filter(|e| e.has_schemas() || e.has_migrations())
            .collect())
    }

    pub fn enabled_job_extensions(
        &self,
        disabled_ids: &[String],
    ) -> Result<Vec<Arc<dyn Extension>>, LoaderError> {
        Ok(self
            .enabled_extensions(disabled_ids)?
            .into_iter()
            .filter(|e| e.has_jobs())
            .collect())
    }

    #[must_use]
    pub fn api_routers(
        &self,
        ctx: &dyn crate::ExtensionContext,
    ) -> Vec<(Arc<dyn Extension>, crate::ExtensionRouter)> {
        self.sorted_extensions
            .iter()
            .filter_map(|e| e.router(ctx).map(|r| (Arc::clone(e), r)))
            .collect()
    }

    #[must_use]
    pub fn job_extensions(&self) -> Vec<Arc<dyn Extension>> {
        self.sorted_extensions
            .iter()
            .filter(|e| e.has_jobs())
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn config_extensions(&self) -> Vec<Arc<dyn Extension>> {
        self.sorted_extensions
            .iter()
            .filter(|e| e.has_config())
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn storage_extensions(&self) -> Vec<Arc<dyn Extension>> {
        self.sorted_extensions
            .iter()
            .filter(|e| e.has_storage_paths())
            .cloned()
            .collect()
    }

    pub fn all_required_storage_paths(&self) -> Vec<&'static str> {
        self.sorted_extensions
            .iter()
            .flat_map(|e| e.required_storage_paths())
            .collect()
    }

    #[must_use]
    pub fn asset_extensions(&self) -> Vec<Arc<dyn Extension>> {
        self.sorted_extensions
            .iter()
            .filter(|e| e.declares_assets())
            .cloned()
            .collect()
    }

    pub fn all_required_assets(
        &self,
        paths: &dyn AssetPaths,
    ) -> Vec<(&'static str, AssetDefinition)> {
        self.sorted_extensions
            .iter()
            .flat_map(|e| {
                let id = e.id();
                e.required_assets(paths)
                    .into_iter()
                    .map(move |asset| (id, asset))
            })
            .collect()
    }

    #[must_use]
    pub fn all_jobs(&self) -> Vec<Arc<dyn Job>> {
        self.sorted_extensions
            .iter()
            .flat_map(|ext| ext.jobs())
            .collect()
    }

    #[must_use]
    pub fn job_by_name(&self, name: &str) -> Option<Arc<dyn Job>> {
        self.sorted_extensions
            .iter()
            .flat_map(|ext| ext.jobs())
            .find(|job| job.name() == name)
    }

    #[must_use]
    pub fn jobs_by_tag(&self, tag: &str) -> Vec<Arc<dyn Job>> {
        self.sorted_extensions
            .iter()
            .flat_map(|ext| ext.jobs())
            .filter(|job| job.tags().contains(&tag))
            .collect()
    }
}
