//! The declaration of the apps an instance vouches for.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::android::{
    ANDROID_ASSET_RELATIONS, AssetLinkNamespace, AssetLinkStatement, AssetLinkTarget, AssetLinks,
};
use super::apple::{
    AppleAppLinkDetail, AppleAppLinks, AppleAppSiteAssociation, AppleLinkComponent,
    AppleWebCredentials,
};
use super::ids::{AppleAppId, AppleTeamId, Sha256CertFingerprint};
use crate::devices::AppIdentifier;

/// The apps an instance vouches for, per vendor. Empty by default: an
/// instance that declares none serves no association document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VouchedApps {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ios: Vec<AppleAppDecl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub android: Vec<AndroidAppDecl>,
}

/// One Apple app: its team and bundle identifier, and the path patterns of
/// the domain it opens. With no `link_paths` the app shares the domain's
/// credentials and opens none of its links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AppleAppDecl {
    pub team_id: AppleTeamId,
    pub bundle_id: AppIdentifier,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub link_paths: Vec<String>,
}

/// One Android app: its package name and the SHA-256 fingerprints of the
/// certificates it may be signed with. At least one fingerprint is required.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AndroidAppDecl {
    pub package_name: AppIdentifier,
    pub sha256_cert_fingerprints: Vec<Sha256CertFingerprint>,
}

/// Why a [`VouchedApps`] declaration is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VouchedAppsError {
    #[error("the iOS app {app_id} is declared more than once")]
    DuplicateAppleApp { app_id: AppleAppId },
    #[error("the iOS app {app_id} has link path {path:?}: it must start with '/'")]
    LinkPathNotAbsolute { app_id: AppleAppId, path: String },
    #[error("the Android app {package_name} is declared more than once")]
    DuplicateAndroidApp { package_name: AppIdentifier },
    #[error("the Android app {package_name} declares no certificate fingerprint")]
    NoFingerprints { package_name: AppIdentifier },
    #[error("the Android app {package_name} lists the fingerprint {fingerprint} more than once")]
    DuplicateFingerprint {
        package_name: AppIdentifier,
        fingerprint: Sha256CertFingerprint,
    },
}

impl AppleAppDecl {
    #[must_use]
    pub fn app_id(&self) -> AppleAppId {
        AppleAppId::new(self.team_id.clone(), self.bundle_id.clone())
    }
}

impl VouchedApps {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.ios.is_empty() && self.android.is_empty()
    }

    pub fn validate(&self) -> Result<(), VouchedAppsError> {
        let mut apple_ids = BTreeSet::new();
        for app in &self.ios {
            let app_id = app.app_id();
            if let Some(path) = app.link_paths.iter().find(|path| !path.starts_with('/')) {
                return Err(VouchedAppsError::LinkPathNotAbsolute {
                    app_id,
                    path: path.clone(),
                });
            }
            if !apple_ids.insert(app_id.clone()) {
                return Err(VouchedAppsError::DuplicateAppleApp { app_id });
            }
        }
        let mut packages = BTreeSet::new();
        for app in &self.android {
            validate_android(app)?;
            if !packages.insert(&app.package_name) {
                return Err(VouchedAppsError::DuplicateAndroidApp {
                    package_name: app.package_name.clone(),
                });
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn apple_app_site_association(&self) -> Option<AppleAppSiteAssociation> {
        if self.ios.is_empty() {
            return None;
        }
        let details = self
            .ios
            .iter()
            .filter(|app| !app.link_paths.is_empty())
            .map(|app| AppleAppLinkDetail {
                app_ids: vec![app.app_id()],
                components: app
                    .link_paths
                    .iter()
                    .map(AppleLinkComponent::matching)
                    .collect(),
            })
            .collect::<Vec<_>>();
        Some(AppleAppSiteAssociation {
            applinks: (!details.is_empty()).then_some(AppleAppLinks { details }),
            webcredentials: AppleWebCredentials {
                apps: self.ios.iter().map(AppleAppDecl::app_id).collect(),
            },
        })
    }

    #[must_use]
    pub fn asset_links(&self) -> Option<AssetLinks> {
        if self.android.is_empty() {
            return None;
        }
        Some(AssetLinks(
            self.android
                .iter()
                .map(|app| AssetLinkStatement {
                    relation: ANDROID_ASSET_RELATIONS.to_vec(),
                    target: AssetLinkTarget {
                        namespace: AssetLinkNamespace::AndroidApp,
                        package_name: app.package_name.clone(),
                        sha256_cert_fingerprints: app.sha256_cert_fingerprints.clone(),
                    },
                })
                .collect(),
        ))
    }
}

fn validate_android(app: &AndroidAppDecl) -> Result<(), VouchedAppsError> {
    if app.sha256_cert_fingerprints.is_empty() {
        return Err(VouchedAppsError::NoFingerprints {
            package_name: app.package_name.clone(),
        });
    }
    let mut seen = BTreeSet::new();
    app.sha256_cert_fingerprints
        .iter()
        .find(|fingerprint| !seen.insert(*fingerprint))
        .map_or(Ok(()), |fingerprint| {
            Err(VouchedAppsError::DuplicateFingerprint {
                package_name: app.package_name.clone(),
                fingerprint: fingerprint.clone(),
            })
        })
}
