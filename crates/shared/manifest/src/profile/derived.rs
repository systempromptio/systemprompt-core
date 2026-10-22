//! Fields derived at load from `server.external_url` and the bind address.
//!
//! A profile states the public URL once. The API URLs, the JWT issuer, the
//! CORS origin and HTTPS flag follow from it; the internal URL and the
//! governance webhook follow from `server.host`/`server.port`. Stating any of
//! them in YAML is refused, because a second copy is a second place to drift.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_yaml::Value;

use super::{AuthzMode, Profile, ProfileError, ProfileResult};

pub const AUTHZ_WEBHOOK_PATH: &str = "/api/public/govern/authz";

pub const DERIVED_FIELDS: &[(&[&str], &str)] = &[
    (&["server", "api_server_url"], "server.external_url"),
    (&["server", "api_external_url"], "server.external_url"),
    (&["server", "use_https"], "server.external_url"),
    (&["server", "cors_allowed_origins"], "server.external_url"),
    (&["security", "jwt_issuer"], "server.external_url"),
    (
        &["server", "api_internal_url"],
        "server.host and server.port",
    ),
    (
        &["governance", "authz", "hook", "url"],
        "server.host and server.port",
    ),
];

fn lookup<'v>(value: &'v Value, path: &[&str]) -> Option<&'v Value> {
    path.iter()
        .try_fold(value, |node, key| node.as_mapping()?.get(*key))
}

pub(super) fn reject_stated_derived_fields(merged: &Value, path: &Path) -> ProfileResult<()> {
    for (field_path, source) in DERIVED_FIELDS {
        if lookup(merged, field_path).is_some() {
            return Err(ProfileError::DerivedField {
                path: path.to_path_buf(),
                field: field_path.join("."),
                source_field: (*source).to_owned(),
            });
        }
    }
    Ok(())
}

pub fn origin_of(url: &url::Url) -> String {
    url.origin().ascii_serialization()
}

#[must_use]
pub fn internal_url_for(host: &str, port: u16) -> String {
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    let reachable = match bare {
        "" | "0.0.0.0" | "::" => "localhost".to_owned(),
        other if other.contains(':') => format!("[{other}]"),
        other => other.to_owned(),
    };
    format!("http://{reachable}:{port}")
}

impl Profile {
    pub fn derive_fields(&mut self) -> ProfileResult<()> {
        let raw = self
            .server
            .external_url
            .trim()
            .trim_end_matches('/')
            .to_owned();
        let parsed = url::Url::parse(&raw).map_err(|e| ProfileError::InvalidExternalUrl {
            url: raw.clone(),
            reason: e.to_string(),
        })?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            return Err(ProfileError::InvalidExternalUrl {
                url: raw,
                reason: "must be an absolute http:// or https:// URL with a host".to_owned(),
            });
        }

        let internal = internal_url_for(&self.server.host, self.server.port);
        let mut origins = vec![origin_of(&parsed)];
        for extra in &self.server.extra_cors_origins {
            if !origins.contains(extra) {
                origins.push(extra.clone());
            }
        }

        self.server.external_url.clone_from(&raw);
        self.server.api_server_url.clone_from(&raw);
        self.server.api_external_url.clone_from(&raw);
        self.server.use_https = parsed.scheme() == "https";
        self.server.cors_allowed_origins = origins;
        self.security.issuer.clone_from(&raw);

        if let Some(hook) = self
            .governance
            .as_mut()
            .and_then(|governance| governance.authz.as_mut())
            .map(|authz| &mut authz.hook)
        {
            hook.url = (hook.mode == AuthzMode::Webhook)
                .then(|| format!("{internal}{AUTHZ_WEBHOOK_PATH}"));
        }
        self.server.api_internal_url = internal;
        Ok(())
    }
}
