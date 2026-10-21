//! What the probe found of a host's managed profile and its application —
//! the two facts the agent verdict is built from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::drift::{DesiredProfile, EntryRole, diff};
use crate::verdict::{Tone, Verdict};

/// Why a complete profile is out of date with the bridge or the gateway.
///
/// Every reason has the same fix, re-applying the profile, so they share a
/// state. They are distinguished only so the message can name the cause; the
/// keys themselves travel with [`ProfileState::Stale`].
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum StaleReason {
    LoopbackSecret,
    ProxyPort,
    ManagedServers,
    ModelList,
    Settings,
}

impl StaleReason {
    // Why: a credential or endpoint that moved breaks routing outright and is
    // named first; a behind model list or policy value still routes.
    fn of(desired: &DesiredProfile, keys: &[String]) -> Self {
        let roles: Vec<EntryRole> = keys.iter().map(|k| desired.role_of(k)).collect();
        [
            (EntryRole::Secret, Self::LoopbackSecret),
            (EntryRole::Endpoint, Self::ProxyPort),
            (EntryRole::ManagedServers, Self::ManagedServers),
            (EntryRole::Models, Self::ModelList),
        ]
        .into_iter()
        .find(|(role, _)| roles.contains(role))
        .map_or(Self::Settings, |(_, reason)| reason)
    }

    #[must_use]
    pub const fn breaks_routing(self) -> bool {
        matches!(self, Self::LoopbackSecret | Self::ProxyPort)
    }
}

/// What a probe hands the classifier: the keys it shows, the same keys
/// unredacted for comparison, and the settings a writer would put there.
///
/// `desired` is an `Err` when the expected settings could not be built (no
/// readable loopback secret, a cache that cannot be read); a profile that
/// cannot be compared is never reported current.
#[derive(Debug)]
pub struct ProfileProbe<'a> {
    pub present: &'a BTreeMap<String, String>,
    pub observed: &'a BTreeMap<String, String>,
    pub read_error: Option<&'a str>,
    pub desired: &'a Result<DesiredProfile, crate::integration::host_app::DesiredError>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileState {
    Absent,
    Partial {
        missing_required: Vec<String>,
    },
    Installed,
    Stale {
        reason: StaleReason,
        keys: Vec<String>,
    },
    Unverifiable {
        reason: String,
    },
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
    Unverifiable,
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
            Self::Unverifiable { .. } => ProfileCode::Unverifiable,
        }
    }

    #[must_use]
    pub const fn tone(&self) -> Tone {
        match self {
            Self::Installed => Tone::Ok,
            Self::Partial { .. } | Self::Stale { .. } | Self::Unverifiable { .. } => Tone::Warn,
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
            Self::Absent | Self::Installed | Self::Stale { .. } | Self::Unverifiable { .. } => &[],
        }
    }

    #[must_use]
    pub fn classify(probe: &ProfileProbe<'_>) -> Self {
        if let Some(reason) = probe.read_error
            && probe.present.is_empty()
        {
            return Self::Unverifiable {
                reason: reason.to_owned(),
            };
        }
        if probe.present.is_empty() {
            return Self::Absent;
        }
        let desired = match probe.desired {
            Ok(desired) => desired,
            Err(reason) => {
                return Self::Unverifiable {
                    reason: format!("the expected settings could not be built: {reason}"),
                };
            },
        };
        let drift = diff(desired, probe.observed);
        let unroutable = drift
            .missing
            .iter()
            .any(|k| matches!(desired.role_of(k), EntryRole::Secret | EntryRole::Endpoint));
        if unroutable {
            return Self::Partial {
                missing_required: drift.missing,
            };
        }
        if drift.is_clean() {
            return Self::Installed;
        }
        let keys = drift.keys();
        Self::Stale {
            reason: StaleReason::of(desired, &keys),
            keys,
        }
    }
}

/// Outcome of looking for the host's application on disk.
///
/// [`Self::Unknown`] is not a synonym for "absent": it means every detector we
/// tried was inconclusive (a bounded probe timed out, a registry hive was
/// unreadable). Callers must never render it as "not installed" — an
/// inconclusive probe must not mask an otherwise healthy host.
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
            Self::NotInstalled => Tone::Unknown,
            Self::Unknown => Tone::Warn,
        }
    }

    #[must_use]
    pub const fn verdict(self) -> Verdict<Self> {
        Verdict::new(self.tone(), self)
    }
}
