//! Push delivery wire shapes: the payload a provider carries and the bodies
//! of the caller's-own-device push token routes.
//!
//! A [`PushPayload`] is all an instance hands a push provider: an opaque
//! notification id, a [`PushCategory`] and a [`CollapseKey`]. It has no title,
//! body or content field, because a payload transits and is stored by a third
//! party; the app shows a fixed line per category and fetches the detail from
//! the instance.
//!
//! A [`PushToken`] is the address a provider issued to one app install. It
//! belongs to the calling device's record: [`RegisterPushTokenRequest`]
//! registers or replaces it and names no device, so a caller cannot write
//! another device's token. The token is a credential for reaching the device
//! and is redacted from `Debug` output.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DeviceId, NotificationId};

use super::push_token::PushToken;
use crate::devices::{DeviceClientInfo, DevicePlatform};

// Why: APNs refuses an `apns-collapse-id` longer than 64 bytes.
const MAX_COLLAPSE_KEY_LEN: usize = 64;

/// The push service a token was issued by.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(type_name = "text", rename_all = "lowercase"))]
#[serde(rename_all = "lowercase")]
pub enum PushProvider {
    Apns,
    Fcm,
}

impl PushProvider {
    pub const ALL: [Self; 2] = [Self::Apns, Self::Fcm];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Apns => "apns",
            Self::Fcm => "fcm",
        }
    }

    #[must_use]
    pub const fn platform(self) -> DevicePlatform {
        match self {
            Self::Apns => DevicePlatform::Ios,
            Self::Fcm => DevicePlatform::Android,
        }
    }
}

impl fmt::Display for PushProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown push provider `{0}`")]
pub struct UnknownPushProvider(pub String);

impl FromStr for PushProvider {
    type Err = UnknownPushProvider;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|provider| provider.as_str() == s)
            .ok_or_else(|| UnknownPushProvider(s.to_owned()))
    }
}

/// What a push is about, as far as the device may know before it asks the
/// instance: the app maps each category to one fixed, localised line.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PushCategory {
    Approval,
    Notification,
}

impl PushCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approval => "approval",
            Self::Notification => "notification",
        }
    }
}

/// The key under which a provider replaces an undelivered push with a newer
/// one: 1 to 64 bytes of ASCII letters, digits, `-`, `_`, `.` and `:`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CollapseKey(String);

/// A string that is not a valid [`CollapseKey`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid collapse key {value:?}: {reason}")]
pub struct CollapseKeyError {
    pub value: String,
    pub reason: &'static str,
}

impl CollapseKey {
    pub fn try_new(value: impl Into<String>) -> Result<Self, CollapseKeyError> {
        let value = value.into();
        match collapse_key_violation(&value) {
            Some(reason) => Err(CollapseKeyError { value, reason }),
            None => Ok(Self(value)),
        }
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

fn collapse_key_violation(value: &str) -> Option<&'static str> {
    if value.is_empty() {
        return Some("must not be empty");
    }
    if value.len() > MAX_COLLAPSE_KEY_LEN {
        return Some("must be at most 64 bytes");
    }
    if !value
        .bytes()
        .all(|b| matches!(b, b'-' | b'_' | b'.' | b':') || b.is_ascii_alphanumeric())
    {
        return Some("must contain only ASCII letters, digits, '-', '_', '.' and ':'");
    }
    None
}

impl fmt::Display for CollapseKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for CollapseKey {
    type Err = CollapseKeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_new(s)
    }
}

crate::notifications::string_form::string_serde!(CollapseKey, "CollapseKey");

/// Everything a push provider is handed for one notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PushPayload {
    pub notification_id: NotificationId,
    pub category: PushCategory,
    pub collapse_key: CollapseKey,
}

impl PushPayload {
    #[must_use]
    pub const fn new(
        notification_id: NotificationId,
        category: PushCategory,
        collapse_key: CollapseKey,
    ) -> Self {
        Self {
            notification_id,
            category,
            collapse_key,
        }
    }
}

/// Body of the register-or-refresh call for the calling device's push token.
/// A device holds one token; a second call replaces the first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegisterPushTokenRequest {
    pub provider: PushProvider,
    pub token: PushToken,
    pub client: DeviceClientInfo,
}

/// Reply to a register-or-refresh call: the device record the token now
/// belongs to. The token is not echoed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PushTokenRegistration {
    pub device_id: DeviceId,
    pub provider: PushProvider,
    pub registered_at: DateTime<Utc>,
}

/// Reply to revoking the calling device's push token. `revoked` is false
/// when the device held none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PushTokenRevocation {
    pub device_id: DeviceId,
    pub revoked: bool,
}
