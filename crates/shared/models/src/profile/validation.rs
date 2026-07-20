//! Profile validation logic.
//!
//! This module contains all validation logic for Profile configurations,
//! including path validation, security settings, CORS, and rate limits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::governance::{AuthzMode, UNRESTRICTED_ACKNOWLEDGEMENT};
use super::security::GATEWAY_REQUIRED_RESOURCE_AUDIENCES;
use super::{Profile, ProfileError, ProfileResult};
use crate::auth::JwtAudience;

impl Profile {
    pub fn validate(&self) -> ProfileResult<()> {
        let mut errors: Vec<String> = Vec::new();
        let is_cloud = self.target.is_cloud();

        self.validate_required_fields(&mut errors);
        self.validate_urls(&mut errors);
        self.validate_paths(&mut errors, is_cloud);
        self.validate_security_settings(&mut errors);
        self.validate_database_pool(&mut errors);
        self.validate_cors_origins(&mut errors);
        self.validate_rate_limits(&mut errors);
        self.validate_governance(&mut errors, is_cloud);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ProfileError::Validation {
                name: self.name.clone(),
                errors,
            })
        }
    }

    pub(super) fn validate_paths(&self, errors: &mut Vec<String>, is_cloud: bool) {
        if is_cloud {
            self.validate_cloud_paths(errors);
        } else {
            self.validate_local_paths(errors);
        }
    }

    pub(super) fn validate_cloud_paths(&self, errors: &mut Vec<String>) {
        Self::require_non_empty(errors, &self.paths.system, "Paths system");
        Self::require_non_empty(errors, &self.paths.services, "Paths services");
        Self::require_non_empty(errors, &self.paths.bin, "Paths bin");

        for (name, path) in [
            ("system", self.paths.system.as_str()),
            ("services", self.paths.services.as_str()),
            ("bin", self.paths.bin.as_str()),
        ] {
            if !path.is_empty() && !path.starts_with("/app") {
                errors.push(format!(
                    "Cloud profile {} path should start with /app, got: {}",
                    name, path
                ));
            }
        }

        if let Some(web_path) = &self.paths.web_path
            && !web_path.is_empty()
        {
            if !web_path.starts_with("/app/web") {
                errors.push(format!(
                    "Cloud profile web_path should start with /app/web, got: {}. Note: \
                         web_path points to the parent of dist/, e.g., /app/web for /app/web/dist",
                    web_path
                ));
            }
            if web_path.contains("/services/web") {
                errors.push(format!(
                    "Cloud profile web_path should be /app/web (for dist output), not \
                         /app/services/web (which is for templates/config). Got: {}",
                    web_path
                ));
            }
        }
    }

    pub(super) fn validate_local_paths(&self, errors: &mut Vec<String>) {
        Self::require_non_empty(errors, &self.paths.system, "Paths system");
        Self::require_non_empty(errors, &self.paths.services, "Paths services");
        Self::require_non_empty(errors, &self.paths.bin, "Paths bin");
    }

    pub(super) fn validate_required_fields(&self, errors: &mut Vec<String>) {
        Self::require_non_empty(errors, &self.name, "Profile name");
        Self::require_non_empty(errors, &self.display_name, "Profile display_name");
        Self::require_non_empty(errors, &self.site.name, "Site name");
        Self::require_non_empty(errors, &self.server.host, "Server host");
        Self::require_non_empty(errors, &self.server.api_server_url, "Server api_server_url");
        Self::require_non_empty(
            errors,
            &self.server.api_internal_url,
            "Server api_internal_url",
        );
        Self::require_non_empty(
            errors,
            &self.server.api_external_url,
            "Server api_external_url",
        );

        if self.server.port == 0 {
            errors.push("Server port must be greater than 0".to_owned());
        }
    }

    pub(super) fn require_non_empty(errors: &mut Vec<String>, value: &str, field_name: &str) {
        if value.is_empty() {
            errors.push(format!("{field_name} is required"));
        }
    }

    pub(super) fn validate_urls(&self, errors: &mut Vec<String>) {
        for (name, value) in [
            ("server.api_server_url", self.server.api_server_url.as_str()),
            (
                "server.api_internal_url",
                self.server.api_internal_url.as_str(),
            ),
            (
                "server.api_external_url",
                self.server.api_external_url.as_str(),
            ),
            ("security.issuer", self.security.issuer.as_str()),
        ] {
            Self::require_absolute_url(errors, name, value, false);
        }

        if !self.server.host.is_empty() && self.server.host.contains("://") {
            errors.push(format!(
                "server.host must be a bare hostname or IP, not a URL (got: {})",
                self.server.host
            ));
        }

        for (idx, issuer) in self.security.trusted_issuers.iter().enumerate() {
            Self::require_absolute_url(
                errors,
                &format!("security.trusted_issuers[{idx}].issuer"),
                &issuer.issuer,
                false,
            );
            Self::require_absolute_url(
                errors,
                &format!("security.trusted_issuers[{idx}].jwks_uri"),
                &issuer.jwks_uri,
                true,
            );
        }

        if let Some(hook) = self.governance.as_ref().and_then(|g| g.authz.as_ref())
            && let Some(url) = hook.hook.url.as_deref()
        {
            Self::require_absolute_url(errors, "governance.authz.hook.url", url, false);
        }
    }

    fn require_absolute_url(errors: &mut Vec<String>, field: &str, value: &str, https_only: bool) {
        if value.is_empty() {
            return;
        }
        let allowed: &[&str] = if https_only {
            &["https"]
        } else {
            &["http", "https"]
        };
        match url::Url::parse(value) {
            Ok(url) if !allowed.contains(&url.scheme()) => {
                errors.push(format!(
                    "{field} must be {} (got scheme '{}': {value})",
                    if https_only {
                        "an https URL"
                    } else {
                        "an http(s) URL"
                    },
                    url.scheme()
                ));
            },
            Ok(url) if url.host_str().is_none_or(str::is_empty) => {
                errors.push(format!("{field} must include a host (got: {value})"));
            },
            Ok(_) => {},
            Err(e) => errors.push(format!("{field} is not a valid URL ({e}): {value}")),
        }
    }

    pub(super) fn validate_security_settings(&self, errors: &mut Vec<String>) {
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

    pub(super) fn validate_database_pool(&self, errors: &mut Vec<String>) {
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

    pub(super) fn validate_governance(&self, errors: &mut Vec<String>, is_cloud: bool) {
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

    pub(super) fn validate_cors_origins(&self, errors: &mut Vec<String>) {
        for origin in &self.server.cors_allowed_origins {
            if origin.is_empty() {
                errors.push("CORS origin cannot be empty".to_owned());
                continue;
            }

            if origin == "*" {
                errors.push("CORS origin '*' is not permitted; list explicit origins".to_owned());
                continue;
            }

            let parsed = match url::Url::parse(origin) {
                Ok(url) => url,
                Err(e) => {
                    errors.push(format!("Invalid CORS origin ({e}): {origin}"));
                    continue;
                },
            };

            let Some(host) = parsed.host_str() else {
                errors.push(format!("CORS origin must include a host: {origin}"));
                continue;
            };

            let bare_host = host.trim_start_matches('[').trim_end_matches(']');
            let is_loopback_http =
                parsed.scheme() == "http" && matches!(bare_host, "localhost" | "127.0.0.1" | "::1");
            if parsed.scheme() != "https" && !is_loopback_http {
                errors.push(format!(
                    "Invalid CORS origin (must be https:// or http://localhost): {origin}"
                ));
                continue;
            }

            if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
                errors.push(format!(
                    "CORS origin must be scheme://host[:port] with no path/query/fragment: {origin}"
                ));
            }
        }
    }

    pub(super) fn validate_rate_limits(&self, errors: &mut Vec<String>) {
        if self.rate_limits.disabled {
            return;
        }

        if self.rate_limits.burst_multiplier == 0 {
            errors.push("rate_limits.burst_multiplier must be greater than 0".to_owned());
        }

        Self::validate_rate_limit(
            errors,
            "oauth_public",
            self.rate_limits.oauth_public_per_second,
        );
        Self::validate_rate_limit(errors, "oauth_auth", self.rate_limits.oauth_auth_per_second);
        Self::validate_rate_limit(errors, "contexts", self.rate_limits.contexts_per_second);
        Self::validate_rate_limit(errors, "tasks", self.rate_limits.tasks_per_second);
        Self::validate_rate_limit(errors, "artifacts", self.rate_limits.artifacts_per_second);
        Self::validate_rate_limit(errors, "agents", self.rate_limits.agents_per_second);
        Self::validate_rate_limit(errors, "mcp", self.rate_limits.mcp_per_second);
        Self::validate_rate_limit(errors, "stream", self.rate_limits.stream_per_second);
        Self::validate_rate_limit(errors, "content", self.rate_limits.content_per_second);
    }

    fn validate_rate_limit(errors: &mut Vec<String>, name: &str, value: u64) {
        if value == 0 {
            errors.push(format!(
                "rate_limits.{}_per_second must be greater than 0",
                name
            ));
        }
    }
}
