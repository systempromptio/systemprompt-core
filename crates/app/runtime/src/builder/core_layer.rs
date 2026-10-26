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
use systemprompt_database::{
    Database, MigrationConfig, SchemaInstallReport, install_extension_schemas_full,
    validate_write_pool_is_primary,
};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_files::FilesConfig;
use systemprompt_governance::authz::SharedAuthzHook;
use systemprompt_loader::{BootContext, ServicesState};
use systemprompt_manifest::services::root::ActiveServicesRoot;
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
    pub(super) market: super::authz::PluginInputs,
}

async fn init_services(
    boot: &BootContext,
    provided: Option<ServicesState>,
) -> RuntimeResult<ServicesState> {
    let secrets = boot.secrets().get();
    let state = if let Some(state) = provided {
        state
    } else {
        let lookup_secrets = Arc::clone(&secrets);
        ServicesState::load_with_discovery(boot, move |catalog| {
            Box::pin(discover_vertex_models(catalog, lookup_secrets))
        })
        .await?
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
    boot: &BootContext,
) -> RuntimeResult<(ActiveServicesRoot, Arc<AppPaths>)> {
    let active_root = boot
        .try_resolve_services_root(env!("CARGO_PKG_VERSION"))
        .await?
        .clone();
    let profile = boot.profile();
    let app_paths = Arc::new(AppPaths::from_profile(
        &profile.paths,
        profile.path_resolution(),
        Some(active_root.path.as_path()),
    )?);
    boot.try_init_config(Some(active_root.path.as_path()))?;
    Ok((active_root, app_paths))
}

fn init_files_config(profile: &Profile, app_paths: &AppPaths) -> RuntimeResult<Arc<FilesConfig>> {
    let files_config = FilesConfig::from_profile(profile, app_paths)?;
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

pub(super) async fn init_core(
    boot: &BootContext,
    overrides: CoreOverrides,
) -> RuntimeResult<CoreLayer> {
    let CoreOverrides {
        authz_hook: authz_hook_override,
        signing_keys: signing_keys_override,
        database_url: database_url_override,
        services: services_provided,
    } = overrides;
    let profile = boot.profile();
    let secrets = boot.secrets().get();
    let (active_root, app_paths) = resolve_services_root(boot).await?;
    let files_config = init_files_config(profile, &app_paths)?;
    let services = init_services(boot, services_provided).await?;
    let services_config = services.config();
    let config = match database_url_override {
        Some(url) => {
            let mut config = Config::clone(boot.config()?);
            config.database_url = url;
            config.database_write_url = None;
            config.database_session_url = None;
            Arc::new(config)
        },
        None => Arc::clone(boot.config()?),
    };
    let instance_id = config.instance_id.clone();
    let file_storage =
        crate::storage::init_file_storage(&profile.storage, &app_paths, &instance_id, &secrets)
            .await?;

    let signing_keys = match signing_keys_override {
        Some(keys) => keys,
        None => SigningKeys::resolve(&config.signing_key_path, boot.secrets())?,
    };

    let database = connect_database(profile, &config).await?;

    validate_write_pool_is_primary(&database).await?;

    crate::services_reconcile::reconcile_fetched_services(
        profile,
        &active_root,
        &services_config,
        &database,
    )
    .await?;

    let cache_epochs = CacheEpochs::new();
    let (policy, authz_hook, market) = super::authz::authz_stage(
        profile,
        &database,
        &services,
        authz_hook_override,
        &cache_epochs,
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
        market,
    })
}

pub async fn discover_vertex_models(
    catalog: &mut systemprompt_manifest::services::catalog::CatalogDocument,
    secrets: Arc<systemprompt_manifest::secrets::Secrets>,
) -> systemprompt_manifest::services::DiscoveryReport {
    let lookup = |name: &str| secrets.get(name).cloned();
    systemprompt_loader::vertex_discovery::discover(
        catalog,
        &lookup,
        std::time::Duration::from_secs(10),
    )
    .await
}

pub(super) fn chain_sources(
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
    profile: &Profile,
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
        crate::schema_currency::refuse_breaking(&report.ahead, profile)?;
        crate::schema_currency::warn_ahead(&report.ahead);
        database.mark_schema_changed();
        report
    } else if schema.verify {
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
