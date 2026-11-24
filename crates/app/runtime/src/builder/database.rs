//! Database connection for the core bootstrap layer: the primary, write and
//! session pools sized from the profile's pool settings, divided from
//! `database.connection_budget` when the profile declares one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::{
    ConnectionBudget, Database, PinnedPools, PoolConfig, PoolSeat, PoolShape,
};
use systemprompt_manifest::{Config, Profile};

use crate::error::RuntimeResult;

pub(crate) async fn connect_database(
    profile: &Profile,
    config: &Config,
    seat: PoolSeat,
) -> RuntimeResult<Arc<Database>> {
    let profile_pool = profile.database.pool.as_ref();
    let mut pool_config = pool_config_from_profile(profile_pool, profile.server.role);
    let pinned_session = profile_pool.and_then(|p| p.session_max_connections);
    let mut session_config = pool_config.for_session(pinned_session);
    if let Some(budget) = profile.database.connection_budget {
        let sizes = ConnectionBudget::new(budget).divide(
            seat,
            PoolShape {
                separate_write: config.database_write_url.is_some(),
                separate_session: config.database_session_url.is_some(),
            },
            PinnedPools {
                main: profile_pool.and_then(|p| p.max_connections_for(profile.server.role)),
                session: pinned_session,
            },
        )?;
        pool_config.max_connections = sizes.main;
        session_config.max_connections = sizes.session.max(1);
        tracing::info!(
            budget,
            main = sizes.main,
            main_pools = sizes.main_pools,
            session = sizes.session,
            children = sizes.children,
            "database connection budget divided"
        );
    }
    Ok(Arc::new(
        Database::connect(
            &config.database_url,
            config.database_write_url.as_deref(),
            &pool_config,
        )
        .await?
        .with_session_url(config.database_session_url.as_deref(), &session_config)
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
