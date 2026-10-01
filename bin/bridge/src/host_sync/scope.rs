//! What a host warning is filed under: one host, or the org-plugins tree that
//! every plugin-reading host shares.
//!
//! The wire form is the plain string the warning has always carried — the
//! host id, or `org-plugins` — so the GUI and a persisted summary read it
//! unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use systemprompt_models::bridge::host::HostKind;

const ORG_PLUGINS: &str = "org-plugins";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WarningScope {
    Host(HostKind),
    OrgPlugins,
}

impl WarningScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Host(kind) => kind.as_str(),
            Self::OrgPlugins => ORG_PLUGINS,
        }
    }

    #[must_use]
    pub fn is_host(self, kind: HostKind) -> bool {
        self == Self::Host(kind)
    }
}

impl From<HostKind> for WarningScope {
    fn from(kind: HostKind) -> Self {
        Self::Host(kind)
    }
}

impl fmt::Display for WarningScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown host warning scope `{0}`")]
pub struct UnknownWarningScope(pub String);

impl FromStr for WarningScope {
    type Err = UnknownWarningScope;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == ORG_PLUGINS {
            return Ok(Self::OrgPlugins);
        }
        HostKind::from_str(s)
            .map(Self::Host)
            .map_err(|_| UnknownWarningScope(s.to_owned()))
    }
}

impl serde::Serialize for WarningScope {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for WarningScope {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::from_str(&raw).map_err(serde::de::Error::custom)
    }
}
