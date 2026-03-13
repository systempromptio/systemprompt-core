use super::ExtensionRegistry;
use crate::Extension;
use crate::asset::{AssetDefinition, AssetPaths};
use std::sync::Arc;
use systemprompt_provider_contracts::Job;
use tracing::warn;

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
        let mut exts: Vec<_> = self
            .sorted_extensions
            .iter()
            .filter(|e| e.has_schemas())
            .cloned()
            .collect();
        exts.sort_by_key(|e| e.migration_weight());
        exts
    }

    #[must_use]
    pub fn enabled_extensions(&self, disabled_ids: &[String]) -> Vec<Arc<dyn Extension>> {
        self.sorted_extensions
            .iter()
            .filter(|ext| {
                let id = ext.id();
                if ext.is_required() {
                    if disabled_ids.iter().any(|d| d == id) {
                        warn!(
                            extension = %id,
                            "Cannot disable required extension - ignoring disabled flag"
                        );
                    }
                    return true;
                }
                !disabled_ids.iter().any(|d| d == id)
            })
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn enabled_schema_extensions(&self, disabled_ids: &[String]) -> Vec<Arc<dyn Extension>> {
        let mut exts: Vec<_> = self
            .enabled_extensions(disabled_ids)
            .into_iter()
            .filter(|e| e.has_schemas() || e.has_migrations())
            .collect();
        exts.sort_by_key(|e| e.migration_weight());
        exts
    }

    #[must_use]
    pub fn enabled_api_extensions(
        &self,
        ctx: &dyn crate::ExtensionContext,
        disabled_ids: &[String],
    ) -> Vec<Arc<dyn Extension>> {
        self.enabled_extensions(disabled_ids)
            .into_iter()
            .filter(|e| e.has_router(ctx))
            .collect()
    }

    #[must_use]
    pub fn enabled_job_extensions(&self, disabled_ids: &[String]) -> Vec<Arc<dyn Extension>> {
        self.enabled_extensions(disabled_ids)
            .into_iter()
            .filter(|e| e.has_jobs())
            .collect()
    }

    #[must_use]
    pub fn api_extensions(&self, ctx: &dyn crate::ExtensionContext) -> Vec<Arc<dyn Extension>> {
        self.sorted_extensions
            .iter()
            .filter(|e| e.has_router(ctx))
            .cloned()
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
    pub fn llm_provider_extensions(&self) -> Vec<Arc<dyn Extension>> {
        self.sorted_extensions
            .iter()
            .filter(|e| e.has_llm_providers())
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn tool_provider_extensions(&self) -> Vec<Arc<dyn Extension>> {
        self.sorted_extensions
            .iter()
            .filter(|e| e.has_tool_providers())
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
