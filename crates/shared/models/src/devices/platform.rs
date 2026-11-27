//! The operating system family a device record runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::notifications::push::PushProvider;

/// The operating system family of a device record.
///
/// `Unknown` is the value of a record that never reported a platform.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(type_name = "text", rename_all = "lowercase"))]
#[serde(rename_all = "lowercase")]
pub enum DevicePlatform {
    Ios,
    Android,
    Macos,
    Windows,
    Linux,
    #[default]
    Unknown,
}

impl DevicePlatform {
    pub const ALL: [Self; 6] = [
        Self::Ios,
        Self::Android,
        Self::Macos,
        Self::Windows,
        Self::Linux,
        Self::Unknown,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ios => "ios",
            Self::Android => "android",
            Self::Macos => "macos",
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn is_phone(self) -> bool {
        matches!(self, Self::Ios | Self::Android)
    }

    #[must_use]
    pub const fn push_provider(self) -> Option<PushProvider> {
        match self {
            Self::Ios => Some(PushProvider::Apns),
            Self::Android => Some(PushProvider::Fcm),
            Self::Macos | Self::Windows | Self::Linux | Self::Unknown => None,
        }
    }
}

impl fmt::Display for DevicePlatform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown device platform `{0}`")]
pub struct UnknownDevicePlatform(pub String);

impl FromStr for DevicePlatform {
    type Err = UnknownDevicePlatform;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|platform| platform.as_str() == s)
            .ok_or_else(|| UnknownDevicePlatform(s.to_owned()))
    }
}
