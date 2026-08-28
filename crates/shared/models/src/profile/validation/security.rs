//! Security, database-pool, governance and external-URL profile checks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::Profile;
use super::super::governance::{AuthzMode, UNRESTRICTED_ACKNOWLEDGEMENT};
use super::super::security::GATEWAY_REQUIRED_RESOURCE_AUDIENCES;
use crate::auth::JwtAudience;

impl Profile {
    pub(crate) fn validate_security_settings(&self, errors: &mut Vec<String>) {
        if self.security.access_token_expiration <= 0 {
            errors.push("Security access_token_expiration must be positive".to_owned());
        }

        if self.security.refresh_token_expiration <= 0 {
            errors.push("Security refresh_token_expiration must be positive".to_owned());
        }

        if !self
            .security
            .audiences
            .iter()
            .any(|aud| JwtAudience::FIRST_PARTY.contains(aud))
        {
            errors.push(
                "security.jwt_audiences must include at least one first-party surface \
                 (web, api, a2a, mcp) — session-context token validation pins the `aud` \
                 claim to that set, so tokens minted without one would be rejected on \
                 every request. Add the standard audiences to the profile YAML and restart."
                    .to_owned(),
            );
        }

        for required in GATEWAY_REQUIRED_RESOURCE_AUDIENCES {
            if !self
                .security
                .allowed_resource_audiences
                .iter()
                .any(|allowed| allowed == required)
            {
                errors.push(format!(
                    "security.allowed_resource_audiences must include \"{required}\" — the \
                     gateway issues tokens bound to audience=\"{required}\" for internal protocol \
                     scopes (hook:govern, hook:track). Add it to the profile YAML and restart."
                ));
            }
        }
    }

    pub(crate) fn validate_database_pool(&self, errors: &mut Vec<String>) {
        let Some(pool) = self.database.pool.as_ref() else {
            return;
        };
        if let Some(max) = pool.max_connections
            && !(1..=500).contains(&max)
        {
            errors.push(format!(
                "database.pool.max_connections must be between 1 and 500 (got {max})"
            ));
        }
        if pool.acquire_timeout_secs == Some(0) {
            errors.push("database.pool.acquire_timeout_secs must be greater than 0".to_owned());
        }
    }

    pub(crate) fn validate_governance(&self, errors: &mut Vec<String>, is_cloud: bool) {
        if !is_cloud {
            return;
        }

        let Some(authz) = self.governance.as_ref().and_then(|g| g.authz.as_ref()) else {
            errors.push(
                "governance.authz is required for cloud profiles — without it the gateway boots \
                 with DenyAllHook and denies every request. Add a governance.authz.hook block \
                 (mode: webhook for production) to the profile YAML."
                    .to_owned(),
            );
            return;
        };

        match authz.hook.mode {
            AuthzMode::Webhook if authz.hook.url.as_deref().unwrap_or_default().is_empty() => {
                errors.push(
                    "governance.authz.hook.url is required when mode is webhook — the gateway \
                     POSTs every request to it."
                        .to_owned(),
                );
            },
            AuthzMode::Unrestricted
                if authz.hook.acknowledgement.as_deref() != Some(UNRESTRICTED_ACKNOWLEDGEMENT) =>
            {
                errors.push(format!(
                    "governance.authz.hook.mode=unrestricted requires acknowledgement to equal \
                     \"{UNRESTRICTED_ACKNOWLEDGEMENT}\" — it disables all authorization."
                ));
            },
            _ => {},
        }
    }

    pub(crate) fn validate_external_url_is_reachable(
        &self,
        errors: &mut Vec<String>,
        is_cloud: bool,
    ) {
        if !is_cloud {
            return;
        }
        let Ok(url) = url::Url::parse(&self.server.api_external_url) else {
            return;
        };
        let Some(host) = url.host_str() else {
            return;
        };
        if Self::is_loopback_host(host) {
            errors.push(format!(
                "server.api_external_url must be the address clients dial, not loopback (got: \
                 {}) — a cloud deployment advertises this URL in OAuth token endpoints, MCP \
                 server URLs, and agent cards, so every remote client would resolve it to \
                 itself.",
                self.server.api_external_url
            ));
        }
    }

    fn is_loopback_host(host: &str) -> bool {
        let bare = host.trim_start_matches('[').trim_end_matches(']');
        bare.eq_ignore_ascii_case("localhost")
            || bare
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    }
}
