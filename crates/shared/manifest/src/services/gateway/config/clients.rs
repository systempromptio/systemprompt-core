//! The phone apps an instance declares: the `clients` section of its
//! discovery document and the apps its domain vouches for.
//!
//! [`BridgeClientsSpec`] is the on-disk form of
//! [`DiscoveryClients`]: per phone platform, the app's identifier and the
//! oldest version the instance still supports. An iOS app also names the
//! `https` link its publisher distributes it under; an Android app is
//! downloaded from the instance's own release proxy, so it names no location.
//! Both this section and the [`VouchedApps`] declaration are checked as they
//! are parsed, so a [`BridgeInstanceSpec`](super::BridgeInstanceSpec) read
//! from a services tree never holds a refused value.
//!
//! ```yaml
//! clients:
//!   ios:
//!     app_id: io.example.app
//!     min_version: 1.2.0
//!     link: https://apps.example.com/redeem
//!   android:
//!     app_id: io.example.app
//!     min_version: 1.3.0
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Deserializer, Serialize};
use systemprompt_models::association::VouchedApps;
use systemprompt_models::bridge::clients::{
    AndroidClient, ClientVersionFloors, DiscoveryClients, IosClient,
};
use systemprompt_models::devices::{AppIdentifier, DevicePlatform};

const IOS_LINK_SCHEME: &str = "https";

/// The phone apps of an instance, per platform. A platform with no app is
/// absent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BridgeClientsSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ios: Option<IosClientSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub android: Option<AndroidClientSpec>,
}

/// The iOS app of an instance and the link it is distributed under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IosClientSpec {
    pub app_id: AppIdentifier,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub min_version: Option<semver::Version>,
    pub link: String,
}

/// The Android app of an instance, downloaded from the instance's release
/// proxy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AndroidClientSpec {
    pub app_id: AppIdentifier,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub min_version: Option<semver::Version>,
}

/// Why a [`BridgeClientsSpec`] is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BridgeClientsError {
    #[error("`clients.ios.link` is '{value}': it is not a URL")]
    IosLinkInvalid {
        value: String,
        #[source]
        source: url::ParseError,
    },
    #[error("`clients.ios.link` is '{value}': it must be an https URL with a host")]
    IosLinkNotHttps { value: String },
    #[error(
        "`clients.{platform}.min_version` is '{version}': build metadata ('+...') is ignored by \
         semver precedence, so a minimum version must not carry it"
    )]
    MinVersionBuildMetadata {
        platform: DevicePlatform,
        version: semver::Version,
    },
}

impl BridgeClientsSpec {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.ios.is_none() && self.android.is_none()
    }

    pub fn validate(&self) -> Result<(), BridgeClientsError> {
        if let Some(ios) = self.ios.as_ref() {
            validate_ios_link(&ios.link)?;
        }
        let floors = self.version_floors();
        DevicePlatform::ALL
            .into_iter()
            .find_map(|platform| {
                floors
                    .floor(platform)
                    .filter(|version| !version.build.is_empty())
                    .map(|version| BridgeClientsError::MinVersionBuildMetadata {
                        platform,
                        version: version.clone(),
                    })
            })
            .map_or(Ok(()), Err)
    }

    #[must_use]
    pub fn to_clients(&self) -> DiscoveryClients {
        DiscoveryClients {
            ios: self.ios.as_ref().map(|ios| IosClient {
                app_id: ios.app_id.clone(),
                min_version: ios.min_version.clone(),
                link: ios.link.clone(),
            }),
            android: self.android.as_ref().map(|android| {
                AndroidClient::served_by_instance(
                    android.app_id.clone(),
                    android.min_version.clone(),
                )
            }),
        }
    }

    #[must_use]
    pub fn version_floors(&self) -> ClientVersionFloors {
        ClientVersionFloors {
            ios: self.ios.as_ref().and_then(|ios| ios.min_version.clone()),
            android: self
                .android
                .as_ref()
                .and_then(|android| android.min_version.clone()),
        }
    }

    #[must_use]
    pub fn serves_download(&self, platform: &str) -> bool {
        self.android.is_some() && platform == DevicePlatform::Android.as_str()
    }
}

fn validate_ios_link(link: &str) -> Result<(), BridgeClientsError> {
    let url = url::Url::parse(link).map_err(|source| BridgeClientsError::IosLinkInvalid {
        value: link.to_owned(),
        source,
    })?;
    if url.scheme() == IOS_LINK_SCHEME && url.host_str().is_some_and(|host| !host.is_empty()) {
        Ok(())
    } else {
        Err(BridgeClientsError::IosLinkNotHttps {
            value: link.to_owned(),
        })
    }
}

pub(super) fn deserialize_clients<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BridgeClientsSpec, D::Error> {
    let clients = BridgeClientsSpec::deserialize(deserializer)?;
    clients.validate().map_err(serde::de::Error::custom)?;
    Ok(clients)
}

pub(super) fn deserialize_vouched_apps<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<VouchedApps, D::Error> {
    let apps = VouchedApps::deserialize(deserializer)?;
    apps.validate().map_err(serde::de::Error::custom)?;
    Ok(apps)
}
