//! Prerender context wiring extensions and providers into the template
//! registry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_models::{AppPaths, ContentConfigRaw, WebConfig};
use systemprompt_provider_contracts::ContentDataProvider;
use systemprompt_template_provider::{DynTemplateLoader, DynTemplateProvider, FileSystemLoader};
use systemprompt_templates::{
    CoreTemplateProvider, EmbeddedDefaultsProvider, TemplateRegistry, TemplateRegistryBuilder,
};
use tokio::fs;

use crate::error::{GeneratorResult, PublishError};
use crate::templates::{get_templates_path, load_web_config};

pub(super) struct PrerenderContext {
    pub db_pool: DbPool,
    pub config: ContentConfigRaw,
    pub web_config: WebConfig,
    pub template_registry: TemplateRegistry,
    pub dist_dir: PathBuf,
    pub content_data_providers: Vec<Arc<dyn ContentDataProvider>>,
}

impl std::fmt::Debug for PrerenderContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrerenderContext")
            .field("config", &self.config)
            .field("web_config", &self.web_config)
            .field("template_registry", &self.template_registry)
            .field("dist_dir", &self.dist_dir)
            .field(
                "content_data_providers_count",
                &self.content_data_providers.len(),
            )
            .finish_non_exhaustive()
    }
}

pub(super) async fn load_prerender_context(
    db_pool: DbPool,
    paths: &AppPaths,
) -> GeneratorResult<PrerenderContext> {
    let config_path = paths.system().content_config();

    let yaml_content = fs::read_to_string(&config_path).await.map_err(|source| {
        PublishError::ContentConfigRead {
            path: config_path.to_path_buf(),
            source,
        }
    })?;
    let config: ContentConfigRaw =
        serde_yaml::from_str(&yaml_content).map_err(|source| PublishError::ContentConfigParse {
            path: config_path.to_path_buf(),
            source,
        })?;

    let web_config = load_web_config(paths).await?;

    tracing::debug!(config_path = %config_path.display(), "Loaded config");

    let extension_template_path = get_templates_path(&web_config, paths);
    if !extension_template_path.exists() {
        return Err(PublishError::config(format!(
            "Template directory not found: {}. Configure profile.paths.web_path or \
             web_config.yaml paths.templates",
            extension_template_path.display()
        )));
    }

    let registry_builder = base_registry_builder(&extension_template_path).await?;
    let (registry_builder, mut content_data_providers) = register_extensions(registry_builder)?;

    content_data_providers.sort_by_key(|p| p.priority());

    let template_registry = registry_builder.build_and_init().await?;

    let dist_dir = paths.web().dist().to_path_buf();

    Ok(PrerenderContext {
        db_pool,
        config,
        web_config,
        template_registry,
        dist_dir,
        content_data_providers,
    })
}

async fn base_registry_builder(
    extension_template_path: &std::path::Path,
) -> GeneratorResult<TemplateRegistryBuilder> {
    let extension_provider = CoreTemplateProvider::discover_with_priority(
        extension_template_path,
        CoreTemplateProvider::EXTENSION_PRIORITY,
    )
    .await?;

    let extension_provider: DynTemplateProvider = Arc::new(extension_provider);
    let embedded_defaults: DynTemplateProvider = Arc::new(EmbeddedDefaultsProvider);
    let loader: DynTemplateLoader = Arc::new(FileSystemLoader::with_path(extension_template_path));

    Ok(TemplateRegistryBuilder::new()
        .with_provider(extension_provider)
        .with_provider(embedded_defaults)
        .with_loader(loader))
}

fn register_extensions(
    mut registry_builder: TemplateRegistryBuilder,
) -> GeneratorResult<(TemplateRegistryBuilder, Vec<Arc<dyn ContentDataProvider>>)> {
    let extensions = ExtensionRegistry::discover()?;
    tracing::debug!(
        extension_count = extensions.extensions().len(),
        "Discovered extensions for prerender context"
    );

    let mut content_data_providers: Vec<Arc<dyn ContentDataProvider>> = Vec::new();

    for ext in extensions.extensions() {
        let providers = ext.page_data_providers();
        let prerenderers = ext.page_prerenderers();
        let content_providers = ext.content_data_providers();
        tracing::debug!(
            extension_id = %ext.metadata().id,
            page_provider_count = providers.len(),
            page_prerenderer_count = prerenderers.len(),
            content_data_provider_count = content_providers.len(),
            component_count = ext.component_renderers().len(),
            extender_count = ext.template_data_extenders().len(),
            "Extension providers discovered"
        );

        for component in ext.component_renderers() {
            registry_builder = registry_builder.with_component(component);
        }
        for extender in ext.template_data_extenders() {
            registry_builder = registry_builder.with_extender(extender);
        }
        for provider in providers {
            registry_builder = registry_builder.with_page_provider(provider);
        }
        for prerenderer in prerenderers {
            registry_builder = registry_builder.with_page_prerenderer(prerenderer);
        }
        content_data_providers.extend(content_providers);
    }

    Ok((registry_builder, content_data_providers))
}
