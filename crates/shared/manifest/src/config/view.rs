//! [`ConfigView`]: the runtime configuration an install member reads, a
//! host-sealed seed listed in
//! `systemprompt_extension::reserved::INSTALL_SEEDS`.
//!
//! The view is copied once, at the composition root, from the resolved
//! [`Config`]; it holds no reference back to it. It omits every field that
//! carries or locates a credential: the database URLs (`database_url`,
//! `database_write_url`, `database_session_url`; the install's database
//! handle is already open) and the JWT signing key path
//! (`signing_key_path`). The struct is exhaustive on purpose: a field added to
//! it is a contract amendment and a breaking change.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{Config, InstanceId};
use crate::profile::{
    AuthorizeConfig, ContentNegotiationConfig, SecurityHeadersConfig, TrustedIssuer,
    UiSessionConfig,
};
use systemprompt_models::auth::JwtAudience;

/// The non-secret runtime configuration an install member may read.
#[derive(Debug, Clone)]
pub struct ConfigView {
    pub instance_id: InstanceId,
    pub metrics_port: Option<u16>,
    pub max_concurrent_streams: usize,
    pub role: crate::profile::NodeRole,
    pub advertise_url: Option<String>,
    pub max_in_flight: Option<std::num::NonZeroU32>,
    pub shutdown: crate::profile::ShutdownConfig,
    pub sitename: String,
    pub database_type: String,
    pub github_link: String,
    pub system_path: String,
    pub services_path: String,
    pub bin_path: String,
    pub skills_path: String,
    pub settings_path: String,
    pub content_config_path: String,
    pub geoip_database_path: Option<String>,
    pub web_path: String,
    pub web_config_path: String,
    pub web_metadata_path: String,
    pub host: String,
    pub port: u16,
    pub api_server_url: String,
    pub api_internal_url: String,
    pub api_external_url: String,
    pub jwt_issuer: String,
    pub jwt_access_token_expiration: i64,
    pub jwt_refresh_token_expiration: i64,
    pub jwt_audiences: Vec<JwtAudience>,
    pub allowed_resource_audiences: Vec<String>,
    pub trusted_issuers: Vec<TrustedIssuer>,
    pub id_jag_ttl_secs: i64,
    pub use_https: bool,
    pub rate_limits: crate::profile::RateLimitsConfig,
    pub retention: crate::profile::RetentionConfig,
    pub planes: crate::profile::ConfigPlanesConfig,
    pub cors_allowed_origins: Vec<String>,
    pub trusted_proxies: Vec<ipnet::IpNet>,
    pub is_cloud: bool,
    pub content_negotiation: ContentNegotiationConfig,
    pub security_headers: SecurityHeadersConfig,
    pub allow_registration: bool,
    pub allow_dynamic_client_registration: bool,
    pub api_key_max_lifetime_days: Option<u32>,
    pub login_page_url: Option<String>,
    pub ui_session: UiSessionConfig,
    pub authorize: AuthorizeConfig,
    pub system_admin_username: String,
    pub system_admin_email: Option<systemprompt_identifiers::Email>,
}

impl ConfigView {
    #[must_use]
    pub fn of(config: &Config) -> Self {
        Self {
            instance_id: config.instance_id.clone(),
            metrics_port: config.metrics_port.clone(),
            max_concurrent_streams: config.max_concurrent_streams.clone(),
            role: config.role.clone(),
            advertise_url: config.advertise_url.clone(),
            max_in_flight: config.max_in_flight.clone(),
            shutdown: config.shutdown.clone(),
            sitename: config.sitename.clone(),
            database_type: config.database_type.clone(),
            github_link: config.github_link.clone(),
            system_path: config.system_path.clone(),
            services_path: config.services_path.clone(),
            bin_path: config.bin_path.clone(),
            skills_path: config.skills_path.clone(),
            settings_path: config.settings_path.clone(),
            content_config_path: config.content_config_path.clone(),
            geoip_database_path: config.geoip_database_path.clone(),
            web_path: config.web_path.clone(),
            web_config_path: config.web_config_path.clone(),
            web_metadata_path: config.web_metadata_path.clone(),
            host: config.host.clone(),
            port: config.port.clone(),
            api_server_url: config.api_server_url.clone(),
            api_internal_url: config.api_internal_url.clone(),
            api_external_url: config.api_external_url.clone(),
            jwt_issuer: config.jwt_issuer.clone(),
            jwt_access_token_expiration: config.jwt_access_token_expiration.clone(),
            jwt_refresh_token_expiration: config.jwt_refresh_token_expiration.clone(),
            jwt_audiences: config.jwt_audiences.clone(),
            allowed_resource_audiences: config.allowed_resource_audiences.clone(),
            trusted_issuers: config.trusted_issuers.clone(),
            id_jag_ttl_secs: config.id_jag_ttl_secs.clone(),
            use_https: config.use_https.clone(),
            rate_limits: config.rate_limits.clone(),
            retention: config.retention.clone(),
            planes: config.planes.clone(),
            cors_allowed_origins: config.cors_allowed_origins.clone(),
            trusted_proxies: config.trusted_proxies.clone(),
            is_cloud: config.is_cloud.clone(),
            content_negotiation: config.content_negotiation.clone(),
            security_headers: config.security_headers.clone(),
            allow_registration: config.allow_registration.clone(),
            allow_dynamic_client_registration: config.allow_dynamic_client_registration.clone(),
            api_key_max_lifetime_days: config.api_key_max_lifetime_days.clone(),
            login_page_url: config.login_page_url.clone(),
            ui_session: config.ui_session.clone(),
            authorize: config.authorize.clone(),
            system_admin_username: config.system_admin_username.clone(),
            system_admin_email: config.system_admin_email.clone(),
        }
    }
}
