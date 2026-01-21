use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_models::{AppPaths, ContentConfigRaw};
use systemprompt_template_provider::{DynTemplateLoader, DynTemplateProvider, FileSystemLoader};
use systemprompt_templates::{CoreTemplateProvider, TemplateRegistry, TemplateRegistryBuilder};
use tokio::fs;

use crate::templates::{get_templates_path, load_web_config};

pub struct PrerenderContext {
    pub db_pool: DbPool,
    pub config: ContentConfigRaw,
    pub web_config: serde_yaml::Value,
    pub template_registry: TemplateRegistry,
    pub dist_dir: PathBuf,
}

pub struct HomepageBranding {
    pub org_name: String,
    pub org_url: String,
    pub org_logo: String,
    pub logo_path: String,
    pub favicon_path: String,
    pub twitter_handle: String,
    pub display_sitename: bool,
}

pub fn extract_homepage_branding(
    web_config: &serde_yaml::Value,
    config: &ContentConfigRaw,
) -> HomepageBranding {
    let org = &config.metadata.structured_data.organization;

    let logo_path = web_config
        .get("branding")
        .and_then(|b| b.get("logo"))
        .and_then(|l| l.get("primary"))
        .and_then(|p| p.get("svg"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let favicon_path = web_config
        .get("branding")
        .and_then(|b| b.get("favicon"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let twitter_handle = web_config
        .get("branding")
        .and_then(|b| b.get("twitter_handle"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let display_sitename = web_config
        .get("branding")
        .and_then(|b| b.get("display_sitename"))
        .and_then(serde_yaml::Value::as_bool)
        .unwrap_or(true);

    HomepageBranding {
        org_name: org.name.clone(),
        org_url: org.url.clone(),
        org_logo: org.logo.clone(),
        logo_path,
        favicon_path,
        twitter_handle,
        display_sitename,
    }
}

pub async fn load_prerender_context(db_pool: DbPool) -> Result<PrerenderContext> {
    let paths = AppPaths::get().map_err(|e| anyhow::anyhow!("{}", e))?;
    let config_path = paths.system().content_config();

    let yaml_content = fs::read_to_string(&config_path)
        .await
        .context("Failed to read content config")?;
    let config: ContentConfigRaw =
        serde_yaml::from_str(&yaml_content).context("Failed to parse content config")?;

    let web_config = load_web_config()
        .await
        .context("Failed to load web config")?;

    tracing::debug!(config_path = %config_path.display(), "Loaded config");

    let template_dir = get_templates_path(&web_config)?;
    let extension_template_path = PathBuf::from(&template_dir);
    let core_template_path = paths.system().default_templates();

    let extension_provider = CoreTemplateProvider::discover_with_priority(
        &extension_template_path,
        CoreTemplateProvider::EXTENSION_PRIORITY,
    )
    .await
    .context("Failed to discover extension templates")?;

    let core_provider = if core_template_path.exists() {
        Some(
            CoreTemplateProvider::discover_with_priority(
                &core_template_path,
                CoreTemplateProvider::DEFAULT_PRIORITY,
            )
            .await
            .context("Failed to discover core templates")?,
        )
    } else {
        tracing::debug!(
            path = %core_template_path.display(),
            "Core templates directory not found, skipping fallback"
        );
        None
    };

    let mut loader = FileSystemLoader::with_path(&extension_template_path);
    if core_template_path.exists() {
        loader = loader.add_path(&core_template_path);
    }

    let extension_provider: DynTemplateProvider = Arc::new(extension_provider);
    let loader: DynTemplateLoader = Arc::new(loader);

    let mut registry_builder = TemplateRegistryBuilder::new()
        .with_provider(extension_provider)
        .with_loader(loader);

    if let Some(core_prov) = core_provider {
        let core_prov: DynTemplateProvider = Arc::new(core_prov);
        registry_builder = registry_builder.with_provider(core_prov);
    }

    let extensions = ExtensionRegistry::discover();
    tracing::info!(
        extension_count = extensions.extensions().len(),
        "Discovered extensions for prerender context"
    );

    for ext in extensions.extensions() {
        let providers = ext.page_data_providers();
        tracing::info!(
            extension_id = %ext.metadata().id,
            page_provider_count = providers.len(),
            component_count = ext.component_renderers().len(),
            extender_count = ext.template_data_extenders().len(),
            "Extension page data providers"
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
    }

    let template_registry = registry_builder
        .build_and_init()
        .await
        .context("Failed to initialize template registry")?;

    let dist_dir = AppPaths::get()
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .web()
        .dist()
        .to_path_buf();

    Ok(PrerenderContext {
        db_pool,
        config,
        web_config,
        template_registry,
        dist_dir,
    })
}
