//! Where a credential may go.
//!
//! A PAT, JWT or hook token travels only over https, or plain HTTP to loopback
//! (`127.0.0.1`, `localhost`, `[::1]`), and only to the instance's own origin.
//! Paths the instance advertises (discovery's device link, a token endpoint)
//! are resolved on that origin and refused when they would leave it.
//!
//! The device-link page accepts as `redirect` only `http://127.0.0.1:<port>/…` or
//! `http://localhost:<port>/…` with an explicit port. An instance on this computer announces
//! the MCP servers it proxies at its container address; when both ends are
//! loopback such a server is reached at the instance's own origin. A hook
//! client's token endpoint is used as advertised only on the instance's origin;
//! otherwise its path (or the default token path) on that origin.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use url::{Host, Url};

use crate::protocol::error::OriginError;
use crate::protocol::wire::paths;

const LOOPBACK_HOSTS: [&str; 3] = ["127.0.0.1", "localhost", "[::1]"];

pub const DEFAULT_TOKEN_ENDPOINT_PATH: &str = paths::OAUTH_TOKEN;

#[must_use]
pub fn is_loopback(url: &Url) -> bool {
    url.host_str()
        .is_some_and(|host| LOOPBACK_HOSTS.contains(&host))
}

#[must_use]
pub fn is_transport_secure(url: &Url) -> bool {
    match url.scheme() {
        "https" => true,
        "http" => is_loopback(url),
        _ => false,
    }
}

#[must_use]
pub fn same_origin(a: &Url, b: &Url) -> bool {
    a.origin() == b.origin()
}

pub fn normalize_instance_url(raw: &str) -> Result<String, OriginError> {
    let url = Url::parse(raw.trim()).map_err(OriginError::Parse)?;
    if !is_transport_secure(&url) {
        return Err(OriginError::Insecure);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(OriginError::Credentials);
    }
    Ok(format!(
        "{}{}",
        url.origin().ascii_serialization(),
        url.path().trim_end_matches('/')
    ))
}

pub fn instance_origin(base: &str) -> Result<String, OriginError> {
    let url = Url::parse(base).map_err(OriginError::Parse)?;
    if !is_transport_secure(&url) {
        return Err(OriginError::Insecure);
    }
    Ok(url.origin().ascii_serialization())
}

pub fn resolve_instance_path(base: &str, path: &str) -> Result<Url, OriginError> {
    if !path.starts_with('/') || path.starts_with("//") || path.contains('\\') {
        return Err(OriginError::Path);
    }
    let base_url = Url::parse(base).map_err(OriginError::Parse)?;
    let joined =
        Url::parse(&format!("{}{path}", base.trim_end_matches('/'))).map_err(OriginError::Parse)?;
    if !same_origin(&base_url, &joined) {
        return Err(OriginError::Foreign);
    }
    Ok(joined)
}

#[must_use]
pub fn is_device_link_redirect(url: &Url) -> bool {
    let host_ok = match url.host() {
        Some(Host::Ipv4(address)) => address.octets() == [127, 0, 0, 1],
        Some(Host::Domain(domain)) => domain == "localhost",
        Some(Host::Ipv6(_)) | None => false,
    };
    url.scheme() == "http" && host_ok && url.port().is_some()
}

#[must_use]
pub fn bearer_allowed(server: &Url, instance: &Url) -> bool {
    is_transport_secure(server) && same_origin(server, instance)
}

#[must_use]
pub fn rehome_instance_mcp_url(server: &Url, instance: &Url) -> Url {
    if !server.path().starts_with(paths::INSTANCE_MCP_PREFIX)
        || !is_loopback(server)
        || !is_loopback(instance)
    {
        return server.clone();
    }
    let mut rehomed = instance.clone();
    rehomed.set_path(server.path());
    rehomed.set_query(server.query());
    rehomed.set_fragment(None);
    rehomed
}

#[must_use]
pub fn align_token_endpoint(advertised: Option<&str>, instance: &Url) -> String {
    let parsed = advertised.and_then(|raw| Url::parse(raw).ok());
    let origin = instance.origin().ascii_serialization();
    match parsed {
        Some(url) if same_origin(&url, instance) => url.to_string(),
        Some(url) => format!("{origin}{}", url.path()),
        None => format!("{origin}{DEFAULT_TOKEN_ENDPOINT_PATH}"),
    }
}
