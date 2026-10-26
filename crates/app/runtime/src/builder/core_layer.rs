//! Core bootstrap layer for [`AppContextBuilder`](super::AppContextBuilder).
//!
//! Resolves the profile-driven foundation an
//! [`AppContext`](crate::context::AppContext) is assembled on — config, paths,
//! files, database pool, signing key, authz hook, and logging — plus extension
//! discovery and schema installation. The files configuration is built here
//! from the resolved paths and owned by the context; the profile and config
//! inits are idempotent `OnceLock` guards, so a non-CLI entry (API, tests) can
//! build a context self-sufficiently while a CLI that already ran them sees a
//! no-op.
//!
//! Upstreams publish and retire models without an operator edit, so the
//! catalog is augmented at boot from their live listings
//! (`discover_vertex_models`). That pass is fail-open: an endpoint no catalog
//! source recognises or a missing or unusable credential leaves the YAML
//! catalog exactly as authored. Which providers are discoverable is decided by
//! the loader's catalog sources from the credential their secret parses into —
//! this layer only supplies the secrets and the budget.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_database::{
    Database, MigrationConfig, SchemaInstallReport, install_extension_schemas_full,
    validate_write_pool_is_primary,
};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_files::FilesConfig;
use systemprompt_governance::authz::SharedAuthzHook;
use systemprompt_loader::ServicesState;
use systemprompt_manifest::{Config, Profile};
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{CacheEpochs, FileStorage};

use super::database::connect_database;
use crate::error::RuntimeResult;

pub(super) struct CoreLayer {
    pub(super) config: Arc<Config>,
    pub(super) app_paths: Arc<AppPaths>,
    pub(super) files_config: Arc<FilesConfig>,
    pub(super) database: Arc<Database>,
    pub(super) authz_hook: SharedAuthzHook,
    pub(super) signing_keys: SigningKeys,
    pub(super) file_storage: Arc<dyn FileStorage>,
    pub(super) cache_epochs: CacheEpochs,
    pub(super) log_sink: systemprompt_logging::LogSink,
    pub(super) policy: systemprompt_marketplace::managed::PublicationPolicy,
    pub(super) services: ServicesState,
}

async fn init_services(
    secrets: &systemprompt_manifest::secrets::Secrets,
    provided: Option<ServicesState>,
) -> RuntimeResult<ServicesState> {
    let state = match provided {
        Some(state) => state,
        None => {
            ServicesState::load_with_discovery(|catalog| Box::pin(discover_vertex_models(catalog)))
                .await?
        },
    };
    if let Some(gateway) = state.config().gateway_config() {
        for reference in gateway.unresolved_secret_refs(|name| secrets.get(name).is_some()) {
            tracing::warn!(
                reference = %reference,
                "gateway config names a secret that is not configured; the dependent endpoint answers 503"
            );
        }
    }
    Ok(state)
}

async fn resolve_services_root(
    profile: &Profile,
    secrets: &systemprompt_manifest::secrets::Secrets,
) -> RuntimeResult<(
    &'static systemprompt_loader::ActiveServicesRoot,
    Arc<AppPaths>,
)> {
    let active_root = systemprompt_loader::ServicesSourceBootstrap::try_run(
        profile,
        |name| secrets.get(name).cloned(),
        env!("CARGO_PKG_VERSION"),
    )
    .await?;
    let app_paths = Arc::new(AppPaths::from_profile(
        &profile.paths,
        profile.path_resolution(),
        Some(active_root.path.as_path()),
    )?);
    systemprompt_config::try_init_config(Some(active_root.path.as_path()))?;
    Ok((active_root, app_paths))
}

fn init_files_config(app_paths: &AppPaths) -> RuntimeResult<Arc<FilesConfig>> {
    let files_config = FilesConfig::from_profile(app_paths)?;
    files_config.validate()?;
    files_config.ensure_storage_structure()?;
    Ok(Arc::new(files_config))
}

#[derive(Default)]
pub(super) struct CoreOverrides {
    pub authz_hook: Option<SharedAuthzHook>,
    pub signing_keys: Option<SigningKeys>,
    pub database_url: Option<String>,
    pub services: Option<ServicesState>,
}

fn authz_wiring(
    services: &ServicesState,
    epochs: CacheEpochs,
) -> systemprompt_governance::authz::AuthzWiring {
    systemprompt_governance::authz::AuthzWiring {
        sources: chain_sources(&services.config()),
        epochs,
        services: services.view(),
    }
}

pub(super) async fn init_core(overrides: CoreOverrides) -> RuntimeResult<CoreLayer> {
    let CoreOverrides {
        authz_hook: authz_hook_override,
        signing_keys: signing_keys_override,
        database_url: database_url_override,
        services: services_provided,
    } = overrides;
    let profile = ProfileBootstrap::get()?;
    let secrets = SecretsBootstrap::get()?;
    let (active_root, app_paths) = resolve_services_root(profile, secrets).await?;
    let files_config = init_files_config(&app_paths)?;
    let services = init_services(secrets, services_provided).await?;
    let services_config = services.config();
    let mut config = Config::get()?.clone();
    if let Some(url) = database_url_override {
        config.database_url = url;
        config.database_write_url = None;
        config.database_session_url = None;
    }
    let config = Arc::new(config);
    let instance_id = config.instance_id.clone();
    let file_storage =
        crate::storage::init_file_storage(&profile.storage, &app_paths, &instance_id, secrets)
            .await?;

    let signing_keys = match signing_keys_override {
        Some(keys) => keys,
        None => SigningKeys::resolve(&config.signing_key_path)?,
    };

    let database = connect_database(profile, &config).await?;

    validate_write_pool_is_primary(&database).await?;

    crate::services_reconcile::reconcile_fetched_services(
        profile,
        active_root,
        &services_config,
        &database,
    )
    .await?;

    let policy = systemprompt_marketplace::managed::PublicationPolicy::discover(
        &database,
        services_config.marketplace.clone().unwrap_or_default(),
        services.view(),
    )?;

    let cache_epochs = CacheEpochs::new();
    let authz_hook = systemprompt_governance::authz::build_authz_hook(
        profile.governance.as_ref(),
        database.write_pool(),
        authz_hook_override,
        authz_wiring(&services, cache_epochs.clone()),
    )?;

    systemprompt_logging::OutputState::current()
        .set_console_format(profile.observability.log_format);
    let log_sink = systemprompt_logging::init_logging(&database, Some(instance_id.clone()));

    if config.database_write_url.is_some() {
        tracing::debug!(
            "Database read/write separation enabled: reads from replica, writes to primary"
        );
    }

    Ok(CoreLayer {
        config,
        app_paths,
        files_config,
        database,
        authz_hook,
        signing_keys,
        file_storage,
        cache_epochs,
        log_sink,
        policy,
        services,
    })
}

pub async fn discover_vertex_models(
    catalog: &mut systemprompt_manifest::services::catalog::CatalogDocument,
) -> systemprompt_manifest::services::DiscoveryReport {
    use systemprompt_manifest::services::DiscoveryReport;

    let Ok(secrets) = SecretsBootstrap::get() else {
        tracing::warn!("secret store unavailable; skipping Vertex model discovery");
        return DiscoveryReport::default();
    };
    let lookup = |name: &str| secrets.get(name).cloned();
    systemprompt_loader::vertex_discovery::discover(
        catalog,
        &lookup,
        std::time::Duration::from_secs(10),
    )
    .await
}

fn chain_sources(
    services: &systemprompt_manifest::services::ServicesConfig,
) -> systemprompt_governance::authz::ChainSources {
    systemprompt_governance::authz::ChainSources::from_services(services)
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SchemaPolicy {
    pub install: bool,
    pub verify: bool,
}

pub(super) async fn init_extensions(
    extension_registry: Option<ExtensionRegistry>,
    schema: SchemaPolicy,
    migration_config: MigrationConfig,
    database: &Arc<Database>,
) -> RuntimeResult<(Arc<ExtensionRegistry>, SchemaInstallReport)> {
    let registry = match extension_registry {
        Some(registry) => registry,
        None => ExtensionRegistry::discover()?,
    };
    registry.validate()?;

    let report = if schema.install {
        let report =
            install_extension_schemas_full(&registry, database.session(), &[], migration_config)
                .await?;
        crate::schema_currency::refuse_breaking(&report.ahead, ProfileBootstrap::get()?)?;
        crate::schema_currency::warn_ahead(&report.ahead);
        database.mark_schema_changed();
        report
    } else if schema.verify {
        let profile = ProfileBootstrap::get()?;
        let ahead =
            crate::schema_currency::assert_schema_current(&registry, database.write(), profile)
                .await?;
        SchemaInstallReport {
            ahead,
            ..SchemaInstallReport::default()
        }
    } else {
        SchemaInstallReport::default()
    };

    Ok((Arc::new(registry), report))
}
