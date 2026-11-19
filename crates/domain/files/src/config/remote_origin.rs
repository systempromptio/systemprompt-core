//! A remote origin a rendered HTML page may load from: an `https` origin
//! written exactly as the URL standard serialises it
//! (`https://cdn.example.com`, `https://cdn.example.com:8443`), with no
//! path, query, fragment, userinfo or wildcard and a lowercase ASCII host of
//! letters, digits, dots and hyphens, or an IPv4 address. Anything else is
//! refused, so an entry can never add a directive or a source of its own to
//! the CSP it is written into.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use url::{Host, Url};

use super::rendered_html::RenderedHtmlError;

/// An `https` origin a rendered page may load from, validated so that it
/// is one CSP source expression and nothing more.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RemoteOrigin(String);

impl RemoteOrigin {
    pub fn try_new(origin: impl Into<String>) -> Result<Self, RenderedHtmlError> {
        let origin = origin.into();
        if let Some(reason) = refusal(&origin)? {
            return Err(RenderedHtmlError::NotAnOrigin { origin, reason });
        }
        Ok(Self(origin))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn refusal(origin: &str) -> Result<Option<&'static str>, RenderedHtmlError> {
    if origin.contains('*') {
        return Ok(Some("holds a wildcard"));
    }
    let url = Url::parse(origin).map_err(|source| RenderedHtmlError::Unparsable {
        origin: origin.to_owned(),
        source,
    })?;
    let reason = if url.scheme() != "https" {
        Some("is not https")
    } else if !url.username().is_empty() || url.password().is_some() {
        Some("carries userinfo")
    } else if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        Some("carries a path, query or fragment")
    } else if !plain_host(url.host().as_ref()) {
        Some("has a host other than a lowercase ASCII name or an IPv4 address")
    } else if url.origin().ascii_serialization() != origin {
        Some("is not written as its origin (lowercase, no trailing slash, no default port)")
    } else {
        None
    };
    Ok(reason)
}

fn plain_host(host: Option<&Host<&str>>) -> bool {
    match host {
        Some(Host::Domain(name)) => {
            !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
        },
        Some(Host::Ipv4(_)) => true,
        Some(Host::Ipv6(_)) | None => false,
    }
}

impl TryFrom<String> for RemoteOrigin {
    type Error = RenderedHtmlError;

    fn try_from(origin: String) -> Result<Self, Self::Error> {
        Self::try_new(origin)
    }
}

impl From<RemoteOrigin> for String {
    fn from(origin: RemoteOrigin) -> Self {
        origin.0
    }
}

impl std::fmt::Display for RemoteOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
