//! What a device reports about itself, and the admin device list row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DeviceId, UserId};

use super::app::AppIdentifier;
use super::platform::DevicePlatform;
use crate::notifications::push::PushProvider;

/// The platform, operating-system version and app build a device reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeviceClientInfo {
    pub platform: DevicePlatform,
    pub os_version: String,
    #[schemars(with = "String")]
    pub app_version: semver::Version,
    pub app_id: AppIdentifier,
}

/// One device record as the admin device list shows it.
///
/// A record that never reported its client info carries
/// [`DevicePlatform::Unknown`] and no versions. `push_provider` names the
/// provider of the device's live push token, if it holds one; the token
/// itself is never listed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AdminDeviceRow {
    pub device_id: DeviceId,
    pub user_id: UserId,
    pub label: String,
    #[serde(default)]
    pub platform: DevicePlatform,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub app_version: Option<semver::Version>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<AppIdentifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_provider: Option<PushProvider>,
    pub enrolled_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<DateTime<Utc>>,
}
