//! Subsystem resolution helpers used by [`AppContextBuilder::build`].
//!
//! [`AppContextBuilder::build`]: super::AppContextBuilder::build
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_analytics::{AnalyticsService, FingerprintRepository, GeoIpReader};
use systemprompt_database::{Database, DbPool};
use systemprompt_marketplace::{AllowAllFilter, MarketplaceFilter, discover_filters};
use systemprompt_models::auth::UserRole;
use systemprompt_models::services::{SystemAdmin, SystemAdminConfig};
use systemprompt_models::{AppPaths, Config, ContentConfigRaw, ContentRouting};
use systemprompt_users::UserService;

use crate::context::AppContext;
use crate::error::{RuntimeError, RuntimeResult};

pub(super) struct ContentAnalytics {
    pub(super) geoip_reader: Option<GeoIpReader>,
    pub(super) content_config: Option<Arc<ContentConfigRaw>>,
    pub(super) route_classifier: Arc<systemprompt_models::RouteClassifier>,
    pub(super) analytics_service: Arc<AnalyticsService>,
    pub(super) fingerprint_repo: Option<Arc<FingerprintRepository>>,
}

pub(super) fn assemble_content_analytics(
    config: &Config,
    app_paths: &AppPaths,
    database: &Arc<Database>,
    show_startup_warnings: bool,
) -> RuntimeResult<ContentAnalytics> {
    let geoip_reader = AppContext::load_geoip_database(config, show_startup_warnings);
    let content_config = AppContext::load_content_config(config, app_paths);
    let content_routing = content_routing_from(content_config.as_ref());
    let route_classifier = Arc::new(systemprompt_models::RouteClassifier::new(
        content_routing.clone(),
    ));
    let analytics_service = Arc::new(AnalyticsService::new(
        database,
        geoip_reader.clone(),
        content_routing,
    )?);

    let fingerprint_repo = match FingerprintRepository::new(database) {
        Ok(repo) => Some(Arc::new(repo)),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to initialize fingerprint repository");
            None
        },
    };

    Ok(ContentAnalytics {
        geoip_reader,
        content_config,
        route_classifier,
        analytics_service,
        fingerprint_repo,
    })
}

pub(super) async fn resolve_and_install_system_admin(
    config: &Config,
    users: &Arc<UserService>,
) -> RuntimeResult<Arc<SystemAdmin>> {
    let cfg = SystemAdminConfig {
        username: config.system_admin_username.clone(),
    };
    let resolved = resolve_system_admin(&cfg, users.as_ref()).await?;
    systemprompt_logging::install_log_attribution(resolved.clone());
    Ok(Arc::new(resolved))
}

async fn resolve_system_admin(
    cfg: &SystemAdminConfig,
    users: &UserService,
) -> RuntimeResult<SystemAdmin> {
    let user = users.find_by_name(&cfg.username).await?.ok_or_else(|| {
        RuntimeError::SystemAdminNotFound {
            username: cfg.username.clone(),
        }
    })?;
    if !user.is_active() {
        return Err(RuntimeError::SystemAdminInactive {
            username: cfg.username.clone(),
        });
    }
    let admin_role = UserRole::Admin.as_str();
    if !user.roles.iter().any(|r| r == admin_role) {
        return Err(RuntimeError::SystemAdminMissingRole {
            username: cfg.username.clone(),
        });
    }
    Ok(SystemAdmin::new(user.id, user.name))
}

pub(super) fn build_marketplace_filter(database: &DbPool) -> Arc<dyn MarketplaceFilter> {
    for reg in discover_filters() {
        match (reg.factory)(database) {
            Ok(filter) => {
                tracing::debug!(
                    priority = reg.priority,
                    "marketplace filter registered via inventory; using highest-priority impl",
                );
                return filter;
            },
            Err(err) => {
                tracing::error!(
                    priority = reg.priority,
                    error = %err,
                    "marketplace filter factory failed; trying next candidate",
                );
            },
        }
    }
    let fallback: Arc<dyn MarketplaceFilter> = Arc::new(AllowAllFilter);
    fallback
}

pub(super) fn content_routing_from(
    content_config: Option<&Arc<ContentConfigRaw>>,
) -> Option<Arc<dyn ContentRouting>> {
    let concrete = Arc::clone(content_config?);
    let routing: Arc<dyn ContentRouting> = concrete;
    Some(routing)
}
