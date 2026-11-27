//! The `apple-app-site-association` document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::ids::AppleAppId;

/// The document served at `/.well-known/apple-app-site-association`.
///
/// `applinks` is absent when no declared app opens any path of the domain;
/// `webcredentials` lists every declared app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAppSiteAssociation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applinks: Option<AppleAppLinks>,
    pub webcredentials: AppleWebCredentials,
}

/// The `applinks` member: which apps open which paths of the domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAppLinks {
    pub details: Vec<AppleAppLinkDetail>,
}

/// One `applinks.details` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAppLinkDetail {
    #[serde(rename = "appIDs")]
    pub app_ids: Vec<AppleAppId>,
    pub components: Vec<AppleLinkComponent>,
}

/// One path pattern of an `applinks` entry, written under the vendor's `/`
/// key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleLinkComponent {
    #[serde(rename = "/")]
    pub path: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exclude: bool,
}

impl AppleLinkComponent {
    pub fn matching(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            exclude: false,
        }
    }
}

/// The `webcredentials` member: the apps that may use the domain's passkeys
/// and saved credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleWebCredentials {
    pub apps: Vec<AppleAppId>,
}
