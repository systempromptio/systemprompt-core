//! Database connection for the core bootstrap layer: the primary, write and
//! session pools sized from the profile's pool settings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::{Database, PoolConfig};
use systemprompt_manifest::{Config, Profile};

use crate::error::RuntimeResult;

pub(super) async fn connect_database(
    profile: &Profile,
    config: &Config,
) -> RuntimeResult<Arc<Database>> {
    let pool_config = pool_config_from_profile(profile.database.pool.as_ref(), profile.server.role);
    Ok(Arc::new(
        Database::connect(
            &config.database_url,
            config.database_write_url.as_deref(),
            &pool_config,
        )
        .await?
        .with_session_url(
            config.database_session_url.as_deref(),
            &pool_config.for_session(
                profile
                    .database
                    .pool
                    .as_ref()
                    .and_then(|p| p.session_max_connections),
            ),
        )
        .await?,
    ))
}

fn pool_config_from_profile(
    profile_pool: Option<&systemprompt_manifest::profile::PoolConfig>,
    role: systemprompt_manifest::profile::NodeRole,
) -> PoolConfig {
    use std::time::Duration;

    let mut cfg = PoolConfig::default();
    let Some(p) = profile_pool else {
        return cfg;
    };
    if let Some(max) = p.max_connections_for(role) {
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
