//! Core bootstrap layer for [`AppContextBuilder`](super::AppContextBuilder).
//!
//! Resolves the profile-driven foundation an
//! [`AppContext`](crate::context::AppContext) is assembled on — config, paths,
//! files, database pool, signing key, authz hook, and logging — plus extension
//! discovery and schema installation. The path/files/config inits are
//! idempotent `OnceLock` guards, so a non-CLI entry (API, tests) can build a
//! context self-sufficiently while a CLI that already ran them sees a no-op.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::ProfileBootstrap;
use systemprompt_database::{
    Database, MigrationConfig, PoolConfig, install_extension_schemas_full,
    validate_write_pool_is_primary,
};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_models::{AppPaths, Config};
use systemprompt_security::authz::SharedAuthzHook;
use systemprompt_traits::FileStorage;

use crate::error::{RuntimeError, RuntimeResult};

pub(super) struct CoreLayer {
    pub(super) config: Arc<Config>,
    pub(super) app_paths: Arc<AppPaths>,
    pub(super) database: Arc<Database>,
    pub(super) authz_hook: SharedAuthzHook,
    pub(super) file_storage: Arc<dyn FileStorage>,
}

pub(super) async fn init_core(
    authz_hook_override: Option<SharedAuthzHook>,
) -> RuntimeResult<CoreLayer> {
    let profile = ProfileBootstrap::get()?;
    let app_paths = Arc::new(AppPaths::from_profile(
        &profile.paths,
        profile.path_resolution(),
    )?);
    systemprompt_files::FilesConfig::init(&app_paths)?;
    systemprompt_config::try_init_config()
        .map_err(|err| RuntimeError::Internal(format!("config init: {err}")))?;
    let config = Arc::new(Config::get()?.clone());
    let instance_id = systemprompt_identifiers::InstanceId::new(&config.instance_id);
    systemprompt_logging::set_instance_id(instance_id.clone());
    let file_storage = init_file_storage(&profile.storage, &app_paths, &instance_id).await?;

    systemprompt_security::keys::authority::init()
        .map_err(|err| RuntimeError::Internal(format!("signing key init: {err}")))?;

    let pool_config = pool_config_from_profile(profile.database.pool.as_ref());
    let database = Arc::new(
        Database::from_config_with_write(
            &config.database_type,
            &config.database_url,
            config.database_write_url.as_deref(),
            &pool_config,
        )
        .await?,
    );

    validate_write_pool_is_primary(&database).await?;

    let authz_audit_pool = database.write_pool_arc().ok();
    let authz_hook = systemprompt_security::authz::build_authz_hook(
        profile.governance.as_ref(),
        authz_audit_pool,
        authz_hook_override,
        chain_sources(),
    )
    .map_err(|err| RuntimeError::Internal(format!("authz bootstrap: {err}")))?;

    systemprompt_logging::init_logging(Arc::clone(&database));

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
        file_storage,
    })
}

async fn init_file_storage(
    storage: &systemprompt_models::profile::StorageConfig,
    app_paths: &AppPaths,
    instance_id: &systemprompt_identifiers::InstanceId,
) -> RuntimeResult<Arc<dyn FileStorage>> {
    let root = app_paths.storage().root();
    let report = systemprompt_storage::probe_shared_mount(root, instance_id)
        .await
        .map_err(|err| {
            RuntimeError::Internal(format!("storage root {} probe: {err}", root.display()))
        })?;
    if !report.write_read_ok {
        return Err(RuntimeError::Internal(format!(
            "storage root {} did not read back what was written",
            root.display()
        )));
    }
    match (storage.shared, report.has_siblings()) {
        (true, false) => tracing::warn!(
            root = %root.display(),
            "storage.shared is true but no other replica has marked this root; \
             it may be a per-node disk"
        ),
        (false, true) => tracing::warn!(
            root = %root.display(),
            instances = ?report.instances,
            "storage.shared is false but other replicas have marked this root; \
             set storage.shared: true if it is a shared mount"
        ),
        _ => {},
    }
    Ok(systemprompt_storage::build_file_storage(
        storage.backend,
        root,
    ))
}

fn chain_sources() -> systemprompt_security::authz::ChainSources {
    match systemprompt_loader::ConfigLoader::load() {
        Ok(services) => systemprompt_security::authz::ChainSources::from_services(&services),
        Err(error) => {
            tracing::warn!(%error, "services config unavailable; authz resolves without parent cascade");
            systemprompt_security::authz::ChainSources::default()
        },
    }
}

fn pool_config_from_profile(
    profile_pool: Option<&systemprompt_models::profile::PoolConfig>,
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
    cfg
}

pub(super) async fn init_extensions(
    extension_registry: Option<ExtensionRegistry>,
    install_schemas: bool,
    migration_config: MigrationConfig,
    database: &Arc<Database>,
) -> RuntimeResult<Arc<ExtensionRegistry>> {
    let registry = match extension_registry {
        Some(registry) => registry,
        None => ExtensionRegistry::discover()?,
    };
    registry.validate()?;

    if install_schemas {
        install_extension_schemas_full(&registry, database.write(), &[], migration_config).await?;
    }

    Ok(Arc::new(registry))
}
