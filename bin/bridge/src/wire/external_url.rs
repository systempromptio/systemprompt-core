//! Guard for URLs the GUI hands to the operating system's default browser.
//!
//! Only absolute `https://` URLs may leave the webview, plus plain `http://`
//! when the host is the machine itself (a local gateway on `localhost`); every
//! other scheme (`javascript:`, `file:`, remote `http:`, custom protocols) is
//! refused before the target reaches any launcher.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalUrl(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExternalUrlRejected {
    #[error("refusing to open non-https url: {0}")]
    NotHttps(String),
    #[error("refusing to open unparseable url: {0}")]
    Malformed(String),
    #[error("refusing to open url with control characters")]
    ControlCharacters,
}

impl ExternalUrl {
    pub fn parse(target: &str) -> Result<Self, ExternalUrlRejected> {
        if target.chars().any(char::is_control) {
            return Err(ExternalUrlRejected::ControlCharacters);
        }
        let parsed = url::Url::parse(target)
            .map_err(|_source| ExternalUrlRejected::Malformed(target.to_owned()))?;
        let allowed = match parsed.scheme() {
            "https" => parsed.host_str().is_some(),
            "http" => parsed.host().as_ref().is_some_and(is_loopback_host),
            _ => false,
        };
        if !allowed {
            return Err(ExternalUrlRejected::NotHttps(target.to_owned()));
        }
        Ok(Self(target.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Why: a gateway on the developer's own machine is reached over plain http;
// that never leaves the host, unlike http to any other name.
const fn is_loopback_host(host: &url::Host<&str>) -> bool {
    match host {
        url::Host::Domain(name) => name.eq_ignore_ascii_case("localhost"),
        url::Host::Ipv4(ip) => ip.is_loopback(),
        url::Host::Ipv6(ip) => ip.is_loopback(),
    }
}

impl fmt::Display for ExternalUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
