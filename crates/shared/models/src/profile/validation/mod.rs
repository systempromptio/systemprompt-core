//! Profile validation logic.
//!
//! This module contains all validation logic for Profile configurations,
//! including path validation, security settings, CORS, and rate limits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod network;
mod security;

use super::{Profile, ProfileError, ProfileResult};

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
        self.validate_external_url_is_reachable(&mut errors, is_cloud);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ProfileError::Validation {
                name: self.name.clone(),
                errors,
            })
        }
    }

    pub(crate) fn validate_paths(&self, errors: &mut Vec<String>, is_cloud: bool) {
        if is_cloud {
            self.validate_cloud_paths(errors);
        } else {
            self.validate_local_paths(errors);
        }
    }

    pub(crate) fn validate_cloud_paths(&self, errors: &mut Vec<String>) {
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

    pub(crate) fn validate_local_paths(&self, errors: &mut Vec<String>) {
        Self::require_non_empty(errors, &self.paths.system, "Paths system");
        Self::require_non_empty(errors, &self.paths.services, "Paths services");
        Self::require_non_empty(errors, &self.paths.bin, "Paths bin");
    }

    pub(crate) fn validate_required_fields(&self, errors: &mut Vec<String>) {
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

    pub(crate) fn require_non_empty(errors: &mut Vec<String>, value: &str, field_name: &str) {
        if value.is_empty() {
            errors.push(format!("{field_name} is required"));
        }
    }

    pub(crate) fn validate_urls(&self, errors: &mut Vec<String>) {
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
}
