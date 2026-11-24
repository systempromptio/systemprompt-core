//! Subsystem resolution helpers used by [`AppContextBuilder::build`].
//!
//! [`AppContextBuilder::build`]: super::AppContextBuilder::build
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::{SystemAdmin, SystemAdminConfig};
use systemprompt_models::{ContentConfigRaw, ContentRouting};
use systemprompt_traits::DynAdminBootstrap;

use crate::error::{RuntimeError, RuntimeResult};

pub(super) struct ContentRoutingParts {
    pub(super) content_config: Option<Arc<ContentConfigRaw>>,
    pub(super) route_classifier: Arc<systemprompt_models::RouteClassifier>,
}

pub(super) fn assemble_content(config: &Config, app_paths: &AppPaths) -> ContentRoutingParts {
    let content_config = crate::context::load_content_config(config, app_paths);
    let content_routing = content_routing_from(content_config.as_ref());
    let route_classifier = Arc::new(systemprompt_models::RouteClassifier::new(content_routing));
    ContentRoutingParts {
        content_config,
        route_classifier,
    }
}

pub(crate) async fn load_system_admin(
    config: &Config,
    admins: &DynAdminBootstrap,
) -> RuntimeResult<Arc<SystemAdmin>> {
    let cfg = SystemAdminConfig {
        username: config.system_admin_username.clone(),
        email: config.system_admin_email.clone(),
    };
    let resolved = resolve_system_admin(&cfg, admins).await?;
    Ok(Arc::new(resolved))
}

async fn resolve_system_admin(
    cfg: &SystemAdminConfig,
    admins: &DynAdminBootstrap,
) -> RuntimeResult<SystemAdmin> {
    let user = admins.find_by_name(&cfg.username).await?.ok_or_else(|| {
        RuntimeError::SystemAdminNotFound {
            username: cfg.username.clone(),
        }
    })?;
    if !user.active {
        return Err(RuntimeError::SystemAdminInactive {
            username: cfg.username.clone(),
        });
    }
    if !user.is_admin() {
        return Err(RuntimeError::SystemAdminMissingRole {
            username: cfg.username.clone(),
        });
    }
    Ok(SystemAdmin::new(user.id, user.name))
}

pub(super) fn content_routing_from(
    content_config: Option<&Arc<ContentConfigRaw>>,
) -> Option<Arc<dyn ContentRouting>> {
    let concrete = Arc::clone(content_config?);
    let routing: Arc<dyn ContentRouting> = concrete;
    Some(routing)
}
