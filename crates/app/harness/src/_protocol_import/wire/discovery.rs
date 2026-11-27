//! `GET /.well-known/systemprompt-bridge` (PROTOCOL section 1). An instance
//! before 0.76 answers `404`; the client then falls back to
//! `paths::FALLBACK_DEVICE_LINK`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

pub const DISCOVERY_CONTRACT: &str = "1";

/// The public discovery document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Discovery {
    pub contract: String,
    pub device_link_path: String,
    #[serde(default)]
    pub identity_path: Option<String>,
    #[serde(default)]
    pub branding: Option<DiscoveryBranding>,
    #[serde(default)]
    pub enabled_hosts: Vec<String>,
}

impl Discovery {
    #[must_use]
    pub fn is_compatible(&self) -> bool {
        self.contract == DISCOVERY_CONTRACT
    }
}

/// The instance's branding in the discovery document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryBranding {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub logo_url: Option<String>,
    #[serde(default)]
    pub sign_in_hint: Option<String>,
    #[serde(default)]
    pub docs_url: Option<String>,
    #[serde(default)]
    pub contact_email: Option<String>,
}
