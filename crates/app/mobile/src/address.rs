//! [`InstanceAddress`], the address a person types or scans for an instance.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::IpAddr;

use url::{Host, Url};

use crate::error::MobileError;

/// The origin of an instance. HTTPS always; plain HTTP only for a loopback or
/// private-network host, which is how a development instance is reached from
/// an emulator or a phone on the same network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceAddress(String);

impl InstanceAddress {
    pub fn parse(input: &str) -> Result<Self, MobileError> {
        let url = Url::parse(input.trim()).map_err(|_| MobileError::InvalidAddress)?;
        let names_only_an_origin = url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && matches!(url.path(), "" | "/");
        let admitted = match (url.scheme(), url.host()) {
            ("https", Some(_)) => true,
            ("http", Some(host)) => is_local(&host),
            _ => false,
        };
        if !(names_only_an_origin && admitted) {
            return Err(MobileError::InvalidAddress);
        }
        Ok(Self(url.origin().ascii_serialization()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn join(&self, path: &str) -> String {
        format!("{}{path}", self.0)
    }
}

fn is_local(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(name) => *name == "localhost",
        Host::Ipv4(ip) => ip.is_loopback() || ip.is_private(),
        Host::Ipv6(ip) => IpAddr::V6(*ip).is_loopback(),
    }
}
