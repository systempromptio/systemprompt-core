//! What the probe found of a host's managed profile and its application —
//! the two facts the agent verdict is built from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::verdict::{Tone, Verdict};

/// Why a complete profile still cannot work.
///
/// Both reasons produce an identical symptom — every request 403s with "bad
/// loopback secret" — and an identical fix, so they share a state. They are
/// distinguished only so the message can name the cause.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum StaleReason {
    LoopbackSecret,
    ProxyPort,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileState {
    Absent,
    Partial { missing_required: Vec<String> },
    Installed,
    Stale { reason: StaleReason },
}

/// [`ProfileState`] without its payload — the code the GUI looks up.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum ProfileCode {
    Absent,
    Partial,
    Installed,
    Stale,
}

impl ProfileState {
    #[must_use]
    pub const fn is_installed(&self) -> bool {
        matches!(self, Self::Installed)
    }

    #[must_use]
    pub const fn code(&self) -> ProfileCode {
        match self {
            Self::Absent => ProfileCode::Absent,
            Self::Partial { .. } => ProfileCode::Partial,
            Self::Installed => ProfileCode::Installed,
            Self::Stale { .. } => ProfileCode::Stale,
        }
    }

    #[must_use]
    pub const fn tone(&self) -> Tone {
        match self {
            Self::Installed => Tone::Ok,
            Self::Partial { .. } | Self::Stale { .. } => Tone::Warn,
            Self::Absent => Tone::Err,
        }
    }

    #[must_use]
    pub const fn verdict(&self) -> Verdict<ProfileCode> {
        Verdict::new(self.tone(), self.code())
    }

    #[must_use]
    pub fn missing_required(&self) -> &[String] {
        match self {
            Self::Partial { missing_required } => missing_required,
            Self::Absent | Self::Installed | Self::Stale { .. } => &[],
        }
    }

    #[must_use]
    pub fn classify(
        required: &[&str],
        present: &BTreeMap<String, String>,
        secret_fresh: Option<bool>,
        endpoint_fresh: Option<bool>,
    ) -> Self {
        match Self::from_keys(required, present) {
            Self::Installed if secret_fresh == Some(false) => Self::Stale {
                reason: StaleReason::LoopbackSecret,
            },
            Self::Installed if endpoint_fresh == Some(false) => Self::Stale {
                reason: StaleReason::ProxyPort,
            },
            state => state,
        }
    }

    #[must_use]
    pub fn endpoint_freshness(configured_url: Option<&str>, proxy_port: u16) -> Option<bool> {
        use crate::proxy_probe::{PortMatch, classify_configured_port};
        let url = configured_url.filter(|u| !u.is_empty())?;
        match classify_configured_port(url, proxy_port) {
            PortMatch::Match => Some(true),
            PortMatch::Mismatch { .. } => Some(false),
            PortMatch::NotLoopback | PortMatch::Unparseable => None,
        }
    }

    fn from_keys(required: &[&str], present: &BTreeMap<String, String>) -> Self {
        if present.is_empty() {
            return Self::Absent;
        }
        let missing: Vec<String> = required
            .iter()
            .filter(|k| !present.contains_key(**k))
            .map(|k| (*k).to_owned())
            .collect();
        if missing.is_empty() {
            Self::Installed
        } else {
            Self::Partial {
                missing_required: missing,
            }
        }
    }
}

/// Outcome of looking for the host's application on disk.
///
/// [`Self::Unknown`] is not a synonym for "absent": it means every detector we
/// tried was inconclusive (a bounded probe timed out, a registry hive was
/// unreadable). Callers must never render it as "not installed" — an
/// inconclusive probe masking an otherwise healthy host is the bug this
/// tri-state exists to prevent.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum AppInstallState {
    Installed,
    NotInstalled,
    Unknown,
}

impl AppInstallState {
    #[must_use]
    pub const fn is_installed(self) -> bool {
        matches!(self, Self::Installed)
    }

    #[must_use]
    pub const fn tone(self) -> Tone {
        match self {
            Self::Installed => Tone::Ok,
            Self::NotInstalled => Tone::Err,
            Self::Unknown => Tone::Warn,
        }
    }

    #[must_use]
    pub const fn verdict(self) -> Verdict<Self> {
        Verdict::new(self.tone(), self)
    }
}
