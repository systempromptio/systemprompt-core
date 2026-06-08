//! Profile validation logic.
//!
//! This module contains all validation logic for Profile configurations,
//! including path validation, security settings, CORS, and rate limits.

use super::governance::{AuthzMode, UNRESTRICTED_ACKNOWLEDGEMENT};
use super::security::GATEWAY_REQUIRED_RESOURCE_AUDIENCES;
use super::{Profile, ProfileError, ProfileResult};

impl Profile {
    pub fn validate(&self) -> ProfileResult<()> {
        let mut errors: Vec<String> = Vec::new();
        let is_cloud = self.target.is_cloud();

        self.validate_required_fields(&mut errors);
        self.validate_paths(&mut errors, is_cloud);
        self.validate_security_settings(&mut errors);
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

        if let Some(web_path) = &self.paths.web_path {
            if !web_path.is_empty() {
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

    pub(super) fn validate_security_settings(&self, errors: &mut Vec<String>) {
        if self.security.access_token_expiration <= 0 {
            errors.push("Security access_token_expiration must be positive".to_owned());
        }

        if self.security.refresh_token_expiration <= 0 {
            errors.push("Security refresh_token_expiration must be positive".to_owned());
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

            let is_https = origin.starts_with("https://");
            let is_loopback_http = origin.starts_with("http://localhost")
                || origin.starts_with("http://127.0.0.1")
                || origin.starts_with("http://[::1]");
            if !is_https && !is_loopback_http {
                errors.push(format!(
                    "Invalid CORS origin (must be https:// or http://localhost): {origin}"
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
