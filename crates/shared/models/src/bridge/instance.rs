//! Per-instance bridge wire shapes: the public discovery document, the
//! instance branding and the release window an instance admits.
//!
//! Discovery is served unauthenticated at
//! `/.well-known/systemprompt-bridge` so a freshly installed bridge can find
//! the instance's sign-in and identity routes before it holds a token. It
//! carries no trust material: the manifest signing key still arrives through
//! managed policy or an operator pin. Branding from discovery is presentation
//! only and yields to the branding inside the signed manifest once one exists.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::bridge::host::HostKind;

pub const DISCOVERY_CONTRACT: &str = "1";

/// The document served at `/.well-known/systemprompt-bridge`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeDiscovery {
    pub contract: String,
    pub device_link_path: String,
    pub identity_path: String,
    #[serde(default)]
    pub branding: InstanceBranding,
    #[serde(default)]
    pub enabled_hosts: Vec<HostKind>,
}

/// What a bridge shows for an instance: window, tray and sign-in copy.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceBranding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_css_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_in_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_email: Option<String>,
}

impl InstanceBranding {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// The bridge release versions an instance admits, compared by strict semver
/// precedence against the bridge's release (brand) version.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeVersionWindow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<semver::Version>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<semver::Version>,
    #[serde(default)]
    pub enforce_max: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowStatus {
    Below,
    Within,
    Above,
    Unknown,
}

impl BridgeVersionWindow {
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    #[must_use]
    pub fn admits(&self, version: &semver::Version) -> bool {
        self.min
            .as_ref()
            .is_none_or(|min| version.cmp_precedence(min).is_ge())
            && self
                .max
                .as_ref()
                .is_none_or(|max| version.cmp_precedence(max).is_le())
    }

    #[must_use]
    pub fn status(&self, reported: &str) -> WindowStatus {
        let Ok(version) = semver::Version::parse(reported) else {
            return WindowStatus::Unknown;
        };
        if self
            .min
            .as_ref()
            .is_some_and(|min| version.cmp_precedence(min).is_lt())
        {
            WindowStatus::Below
        } else if self
            .max
            .as_ref()
            .is_some_and(|max| version.cmp_precedence(max).is_gt())
        {
            WindowStatus::Above
        } else {
            WindowStatus::Within
        }
    }

    #[must_use]
    pub fn intersect(&self, other: &Self) -> Self {
        let min = match (&self.min, &other.min) {
            (Some(a), Some(b)) => Some(if a.cmp_precedence(b).is_ge() {
                a.clone()
            } else {
                b.clone()
            }),
            (a, b) => a.clone().or_else(|| b.clone()),
        };
        let max = match (&self.max, &other.max) {
            (Some(a), Some(b)) => Some(if a.cmp_precedence(b).is_le() {
                a.clone()
            } else {
                b.clone()
            }),
            (a, b) => a.clone().or_else(|| b.clone()),
        };
        Self {
            min,
            max,
            enforce_max: self.enforce_max || other.enforce_max,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!((&self.min, &self.max), (Some(min), Some(max)) if min.cmp_precedence(max).is_gt())
    }
}

/// Whether a bridge is carrying its user's traffic or has been switched off
/// by its user, as reported on the heartbeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeState {
    Active,
    Disabled,
}

impl BridgeState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }
}

impl WindowStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Below => "below",
            Self::Within => "within",
            Self::Above => "above",
            Self::Unknown => "unknown",
        }
    }
}
