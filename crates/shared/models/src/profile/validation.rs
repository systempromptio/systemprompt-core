//! Profile validation logic.
//!
//! This module contains all validation logic for Profile configurations,
//! including path validation, security settings, CORS, and rate limits.

use std::path::Path;

use super::Profile;
use anyhow::Result;

impl Profile {
    /// Validates the entire profile configuration.
    pub fn validate(&self) -> Result<()> {
        let mut errors: Vec<String> = Vec::new();
        let is_cloud = self.target.is_cloud();

        self.validate_required_fields(&mut errors);
        self.validate_paths(&mut errors, is_cloud);
        self.validate_security_settings(&mut errors);
        self.validate_cors_origins(&mut errors);
        self.validate_rate_limits(&mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            anyhow::bail!(
                "Profile '{}' validation failed:\n  - {}",
                self.name,
                errors.join("\n  - ")
            )
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
    }

    pub(super) fn validate_local_paths(&self, errors: &mut Vec<String>) {
        Self::validate_local_required_path(errors, "system", &self.paths.system);
        Self::validate_local_required_path(errors, "services", &self.paths.services);
        Self::validate_local_required_path(errors, "bin", &self.paths.bin);

        Self::validate_local_optional_path(errors, "storage", self.paths.storage.as_ref());
        Self::validate_local_optional_path(
            errors,
            "geoip_database",
            self.paths.geoip_database.as_ref(),
        );
        Self::validate_local_optional_path(errors, "web_path", self.paths.web_path.as_ref());
    }

    fn validate_local_required_path(errors: &mut Vec<String>, name: &str, path: &str) {
        if path.is_empty() {
            errors.push(format!("Paths {} is required", name));
            return;
        }

        if !Path::new(path).exists() {
            errors.push(format!("{} path does not exist: {}", name, path));
        }
    }

    fn validate_local_optional_path(errors: &mut Vec<String>, name: &str, path: Option<&String>) {
        if let Some(p) = path {
            if !p.is_empty() && !Path::new(p).exists() {
                errors.push(format!("paths.{} does not exist: {}", name, p));
            }
        }
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
            errors.push("Server port must be greater than 0".to_string());
        }
    }

    pub(super) fn require_non_empty(errors: &mut Vec<String>, value: &str, field_name: &str) {
        if value.is_empty() {
            errors.push(format!("{field_name} is required"));
        }
    }

    pub(super) fn validate_security_settings(&self, errors: &mut Vec<String>) {
        if self.security.access_token_expiration <= 0 {
            errors.push("Security access_token_expiration must be positive".to_string());
        }

        if self.security.refresh_token_expiration <= 0 {
            errors.push("Security refresh_token_expiration must be positive".to_string());
        }
    }

    pub(super) fn validate_cors_origins(&self, errors: &mut Vec<String>) {
        for origin in &self.server.cors_allowed_origins {
            if origin.is_empty() {
                errors.push("CORS origin cannot be empty".to_string());
                continue;
            }

            let is_valid = origin.starts_with("http://") || origin.starts_with("https://");
            if !is_valid {
                errors.push(format!(
                    "Invalid CORS origin (must start with http:// or https://): {}",
                    origin
                ));
            }
        }
    }

    pub(super) fn validate_rate_limits(&self, errors: &mut Vec<String>) {
        if self.rate_limits.disabled {
            return;
        }

        if self.rate_limits.burst_multiplier == 0 {
            errors.push("rate_limits.burst_multiplier must be greater than 0".to_string());
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
