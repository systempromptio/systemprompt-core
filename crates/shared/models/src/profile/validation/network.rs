//! CORS-origin and rate-limit profile checks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::Profile;

impl Profile {
    pub(crate) fn validate_trusted_proxies(&self, errors: &mut Vec<String>, is_cloud: bool) {
        if is_cloud && self.server.trusted_proxies.is_empty() {
            errors.push(
                "server.trusted_proxies is required on a cloud profile: without it forwarded \
                 client-IP headers are ignored and every request resolves to the proxy's peer \
                 address, so all callers share one rate-limit bucket and one ban target"
                    .to_owned(),
            );
        }
    }

    pub(crate) fn validate_cors_origins(&self, errors: &mut Vec<String>) {
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

    pub(crate) fn validate_rate_limits(&self, errors: &mut Vec<String>) {
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
