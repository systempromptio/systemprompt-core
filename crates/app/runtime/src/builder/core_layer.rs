//! Core bootstrap layer for [`AppContextBuilder`](super::AppContextBuilder).
//!
//! Resolves the profile-driven foundation an
//! [`AppContext`](crate::context::AppContext) is assembled on — config, paths,
//! files, database pool, signing key, authz hook, and logging — plus extension
//! discovery and schema installation. The path/files/config inits are
//! idempotent `OnceLock` guards, so a non-CLI entry (API, tests) can build a
//! context self-sufficiently while a CLI that already ran them sees a no-op.
//!
//! Upstreams publish and retire models without an operator edit, so the
//! provider registry is augmented at boot from their live listings
//! (`discover_vertex_models`). That pass is fail-open: a provider no catalog
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
    Database, MigrationConfig, PoolConfig, SchemaInstallReport, install_extension_schemas_full,
    validate_write_pool_is_primary,
};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_manifest::Config;
use systemprompt_security::authz::SharedAuthzHook;
use systemprompt_security::policy::GovernanceEngine;
use systemprompt_traits::FileStorage;

use crate::error::RuntimeResult;

pub(super) struct CoreLayer {
    pub(super) config: Arc<Config>,
    pub(super) app_paths: Arc<AppPaths>,
    pub(super) database: Arc<Database>,
    pub(super) authz_hook: SharedAuthzHook,
    pub(super) governance: Arc<GovernanceEngine>,
    pub(super) file_storage: Arc<dyn FileStorage>,
}

async fn init_services(
    secrets: &systemprompt_manifest::secrets::Secrets,
) -> RuntimeResult<&'static systemprompt_manifest::services::ServicesConfig> {
    let services = systemprompt_loader::ServicesBootstrap::try_init_with_discovery(|providers| {
        Box::pin(discover_vertex_models(providers))
    })
    .await?;
    if let Some(gateway) = services.gateway_config() {
        for reference in gateway.unresolved_secret_refs(|name| secrets.get(name).is_some()) {
            tracing::warn!(
                reference = %reference,
                "gateway config names a secret that is not configured; the dependent endpoint answers 503"
            );
        }
    }
    Ok(services)
}

pub(super) async fn init_core(
    authz_hook_override: Option<SharedAuthzHook>,
) -> RuntimeResult<CoreLayer> {
    let profile = ProfileBootstrap::get()?;
    let secrets = SecretsBootstrap::get()?;
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
    systemprompt_files::FilesConfig::init(&app_paths)?;
    systemprompt_config::try_init_config(Some(active_root.path.as_path()))?;
    let services = init_services(secrets).await?;
    let config = Arc::new(Config::get()?.clone());
    let instance_id = config.instance_id.clone();
    systemprompt_logging::set_instance_id(instance_id.clone());
    let file_storage =
        crate::storage::init_file_storage(&profile.storage, &app_paths, &instance_id, secrets)
            .await?;

    systemprompt_security::keys::authority::init()?;

    let pool_config = pool_config_from_profile(profile.database.pool.as_ref());
    let database = Arc::new(
        Database::connect(
            &config.database_url,
            config.database_write_url.as_deref(),
            &pool_config,
        )
        .await?,
    );

    validate_write_pool_is_primary(&database).await?;

    crate::services_reconcile::reconcile_fetched_services(
        profile,
        active_root,
        services,
        &database,
    )
    .await?;

    let authz_audit_pool = database.write_pool();
    let authz_hook = systemprompt_security::authz::build_authz_hook(
        profile.governance.as_ref(),
        authz_audit_pool,
        authz_hook_override,
        chain_sources()?,
    )?;

    let governance = Arc::new(GovernanceEngine::from_services_root(std::path::Path::new(
        &profile.paths.services,
    ))?);

    systemprompt_logging::init_logging(&database);

    if config.database_write_url.is_some() {
        tracing::debug!(
            "Database read/write separation enabled: reads from replica, writes to primary"
        );
    }

    Ok(CoreLayer {
        config,
        app_paths,
        database,
        authz_hook,
        governance,
        file_storage,
    })
}

pub async fn discover_vertex_models(
    providers: &mut systemprompt_manifest::services::ProviderRegistry,
) -> systemprompt_manifest::services::DiscoveryReport {
    use systemprompt_manifest::services::DiscoveryReport;

    let Ok(secrets) = SecretsBootstrap::get() else {
        tracing::warn!("secret store unavailable; skipping Vertex model discovery");
        return DiscoveryReport::default();
    };
    let lookup = |name: &str| secrets.get(name).cloned();
    systemprompt_loader::vertex_discovery::discover(
        providers,
        &lookup,
        std::time::Duration::from_secs(10),
    )
    .await
}

fn chain_sources() -> RuntimeResult<systemprompt_security::authz::ChainSources> {
    let services = systemprompt_loader::ServicesBootstrap::get()?;
    Ok(systemprompt_security::authz::ChainSources::from_services(
        services,
    ))
}

fn pool_config_from_profile(
    profile_pool: Option<&systemprompt_manifest::profile::PoolConfig>,
) -> PoolConfig {
    use std::time::Duration;

    let mut cfg = PoolConfig::default();
    let Some(p) = profile_pool else {
        return cfg;
    };
    if let Some(max) = p.max_connections {
        cfg.max_connections = max;
    }
    if let Some(secs) = p.acquire_timeout_secs {
        cfg.acquire_timeout = Duration::from_secs(secs);
    }
    if let Some(secs) = p.idle_timeout_secs {
        cfg.idle_timeout = Duration::from_secs(secs);
    }
    if let Some(secs) = p.max_lifetime_secs {
        cfg.max_lifetime = Duration::from_secs(secs);
    }
    if let Some(capacity) = p.statement_cache_capacity {
        cfg.statement_cache_capacity = capacity;
    }
    cfg
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
        install_extension_schemas_full(&registry, database.write(), &[], migration_config).await?
    } else {
        if schema.verify {
            let profile = ProfileBootstrap::get()?;
            crate::schema_currency::assert_schema_current(&registry, database.write(), profile)
                .await?;
        }
        SchemaInstallReport::default()
    };

    Ok((Arc::new(registry), report))
}
