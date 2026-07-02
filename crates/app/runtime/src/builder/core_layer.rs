//! Core bootstrap layer for [`AppContextBuilder`](super::AppContextBuilder).
//!
//! Resolves the profile-driven foundation an
//! [`AppContext`](crate::context::AppContext) is assembled on — config, paths,
//! files, database pool, signing key, authz hook, and logging — plus extension
//! discovery and schema installation. The path/files/config inits are
//! idempotent `OnceLock` guards, so a non-CLI entry (API, tests) can build a
//! context self-sufficiently while a CLI that already ran them sees a no-op.

use std::sync::Arc;

use systemprompt_config::ProfileBootstrap;
use systemprompt_database::{
    Database, MigrationConfig, PoolConfig, install_extension_schemas_full,
};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_models::{AppPaths, Config};
use systemprompt_security::authz::SharedAuthzHook;

use crate::error::{RuntimeError, RuntimeResult};

pub(super) struct CoreLayer {
    pub(super) config: Arc<Config>,
    pub(super) app_paths: Arc<AppPaths>,
    pub(super) database: Arc<Database>,
    pub(super) authz_hook: SharedAuthzHook,
}

pub(super) async fn init_core(
    authz_hook_override: Option<SharedAuthzHook>,
) -> RuntimeResult<CoreLayer> {
    let profile = ProfileBootstrap::get()?;
    let app_paths = Arc::new(AppPaths::from_profile(&profile.paths)?);
    systemprompt_files::FilesConfig::init(&app_paths)?;
    systemprompt_config::try_init_config()
        .map_err(|err| RuntimeError::Internal(format!("config init: {err}")))?;
    let config = Arc::new(Config::get()?.clone());

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

    // Why: the audit pool is optional — without a write pool the authz hook
    // still boots (with a non-persistent audit sink) rather than failing the
    // whole context build, so a degraded boot proceeds without audit persistence.
    let authz_audit_pool = database.write_pool_arc().ok();
    let authz_hook = systemprompt_security::authz::build_authz_hook(
        profile.governance.as_ref(),
        authz_audit_pool,
        authz_hook_override,
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
    })
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
