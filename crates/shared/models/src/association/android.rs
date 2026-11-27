//! The `assetlinks.json` document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::ids::Sha256CertFingerprint;
use crate::devices::AppIdentifier;

pub const ANDROID_ASSET_RELATIONS: [AssetLinkRelation; 2] = [
    AssetLinkRelation::HandleAllUrls,
    AssetLinkRelation::GetLoginCreds,
];

/// The document served at `/.well-known/assetlinks.json`: a JSON array of
/// statements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetLinks(pub Vec<AssetLinkStatement>);

/// One statement: the relations the domain grants one target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetLinkStatement {
    pub relation: Vec<AssetLinkRelation>,
    pub target: AssetLinkTarget,
}

/// What a statement lets its target do with the domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssetLinkRelation {
    #[serde(rename = "delegate_permission/common.handle_all_urls")]
    HandleAllUrls,
    #[serde(rename = "delegate_permission/common.get_login_creds")]
    GetLoginCreds,
}

/// The kind of asset a statement targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetLinkNamespace {
    AndroidApp,
}

/// The Android app a statement targets, identified by its package name and
/// the certificates it may be signed with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetLinkTarget {
    pub namespace: AssetLinkNamespace,
    pub package_name: AppIdentifier,
    pub sha256_cert_fingerprints: Vec<Sha256CertFingerprint>,
}
