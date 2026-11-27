//! The native apps an instance offers, as its discovery document lists them.
//!
//! [`DiscoveryClients`] is the `clients` section of
//! [`BridgeDiscovery`](crate::bridge::instance::BridgeDiscovery): per phone
//! platform, the app's identifier, the oldest version the instance still
//! supports and where to get the app. An Android app is downloaded from the
//! instance itself; an iOS app is reached through a link the publisher
//! distributes it under. [`ClientVersionFloors`] is the per-platform minimum
//! on its own, the phone counterpart of
//! [`BridgeVersionWindow`](crate::bridge::instance::BridgeVersionWindow): a
//! floor is a property of each platform, and a platform with none admits
//! every version.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::bridge::instance::WindowStatus;
use crate::devices::{AppIdentifier, DevicePlatform};
use crate::modules::ApiPaths;

/// The `clients` section of the discovery document. A platform with no app
/// configured is absent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryClients {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ios: Option<IosClient>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub android: Option<AndroidClient>,
}

/// The iOS app of an instance and the link it is distributed under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IosClient {
    pub app_id: AppIdentifier,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_version: Option<semver::Version>,
    pub link: String,
}

/// The Android app of an instance and the path on the instance it downloads
/// from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AndroidClient {
    pub app_id: AppIdentifier,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_version: Option<semver::Version>,
    pub download_path: String,
}

impl AndroidClient {
    #[must_use]
    pub fn served_by_instance(app_id: AppIdentifier, min_version: Option<semver::Version>) -> Self {
        Self {
            app_id,
            min_version,
            download_path: ApiPaths::bridge_download(DevicePlatform::Android.as_str()),
        }
    }
}

impl DiscoveryClients {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.ios.is_none() && self.android.is_none()
    }

    #[must_use]
    pub fn version_floors(&self) -> ClientVersionFloors {
        ClientVersionFloors {
            ios: self.ios.as_ref().and_then(|c| c.min_version.clone()),
            android: self.android.as_ref().and_then(|c| c.min_version.clone()),
        }
    }
}

/// The oldest app version an instance admits on each phone platform,
/// compared by strict semver precedence.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientVersionFloors {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ios: Option<semver::Version>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub android: Option<semver::Version>,
}

impl ClientVersionFloors {
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.ios.is_none() && self.android.is_none()
    }

    #[must_use]
    pub const fn floor(&self, platform: DevicePlatform) -> Option<&semver::Version> {
        match platform {
            DevicePlatform::Ios => self.ios.as_ref(),
            DevicePlatform::Android => self.android.as_ref(),
            DevicePlatform::Macos
            | DevicePlatform::Windows
            | DevicePlatform::Linux
            | DevicePlatform::Unknown => None,
        }
    }

    #[must_use]
    pub fn admits(&self, platform: DevicePlatform, version: &semver::Version) -> bool {
        self.floor(platform)
            .is_none_or(|min| version.cmp_precedence(min).is_ge())
    }

    #[must_use]
    pub fn status(&self, platform: DevicePlatform, reported: &str) -> WindowStatus {
        match semver::Version::parse(reported) {
            Err(_) => WindowStatus::Unknown,
            Ok(version) if self.admits(platform, &version) => WindowStatus::Within,
            Ok(_) => WindowStatus::Below,
        }
    }
}
