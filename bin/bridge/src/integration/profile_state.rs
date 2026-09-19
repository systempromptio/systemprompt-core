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
    ManagedServers,
}

/// Whether one fact a fresh profile depends on could be checked, and what it
/// said.
///
/// `Unchecked` is a host that does not carry the fact at all (a CLI host
/// keeps the secret in a file the probe does not read); `Unverifiable` is a
/// fact the host carries but the probe could not evaluate — a guard that
/// cannot evaluate never reports green.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Freshness {
    Fresh,
    Stale,
    Unchecked,
    Unverifiable { reason: String },
}

impl Freshness {
    #[must_use]
    pub fn compare(installed: Option<&str>, live: Option<&str>, what: &str) -> Self {
        match (installed, live) {
            (Some(installed), Some(live)) if installed == live => Self::Fresh,
            (Some(_), Some(_)) => Self::Stale,
            (None, _) => Self::Unchecked,
            (Some(_), None) => Self::Unverifiable {
                reason: format!("the live {what} could not be read"),
            },
        }
    }
}

#[derive(Debug)]
pub struct ProfileProbe<'a> {
    pub required: &'a [&'a str],
    pub present: &'a BTreeMap<String, String>,
    pub read_error: Option<&'a str>,
    pub secret: Freshness,
    pub endpoint: Freshness,
    pub managed_servers: Freshness,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileState {
    Absent,
    Partial { missing_required: Vec<String> },
    Installed,
    Stale { reason: StaleReason },
    Unverifiable { reason: String },
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
        match Self::from_keys(probe.required, probe.present) {
            Self::Installed => match (&probe.secret, &probe.endpoint, &probe.managed_servers) {
                (Freshness::Stale, _, _) => Self::Stale {
                    reason: StaleReason::LoopbackSecret,
                },
                (_, Freshness::Stale, _) => Self::Stale {
                    reason: StaleReason::ProxyPort,
                },
                (_, _, Freshness::Stale) => Self::Stale {
                    reason: StaleReason::ManagedServers,
                },
                (Freshness::Unverifiable { reason }, _, _)
                | (_, Freshness::Unverifiable { reason }, _)
                | (_, _, Freshness::Unverifiable { reason }) => Self::Unverifiable {
                    reason: reason.clone(),
                },
                _ => Self::Installed,
            },
            state => state,
        }
    }

    #[must_use]
    pub fn endpoint_freshness(configured_url: Option<&str>, proxy_port: u16) -> Freshness {
        use crate::proxy_probe::{PortMatch, classify_configured_port};
        let Some(url) = configured_url.filter(|u| !u.is_empty()) else {
            return Freshness::Unchecked;
        };
        match classify_configured_port(url, proxy_port) {
            PortMatch::Match => Freshness::Fresh,
            PortMatch::Mismatch { .. } => Freshness::Stale,
            PortMatch::NotLoopback => Freshness::Unverifiable {
                reason: format!(
                    "configured gateway url {url} does not point at the loopback proxy"
                ),
            },
            PortMatch::Unparseable => Freshness::Unverifiable {
                reason: format!("configured gateway url {url} cannot be parsed"),
            },
        }
    }

    // Why: the policy names the servers Claude Desktop may reach, and a
    // registry the bridge has never loaded says nothing about them — an
    // unknown expectation is Unchecked, never Stale, so a launch before the
    // first sync cannot re-apply an empty list over a good one.
    #[must_use]
    pub fn managed_servers_freshness(
        installed: Option<&str>,
        expected: Option<&[String]>,
    ) -> Freshness {
        let Some(expected) = expected else {
            return Freshness::Unchecked;
        };
        let mut want: Vec<&str> = expected.iter().map(String::as_str).collect();
        want.sort_unstable();
        want.dedup();
        let Some(installed) = installed.map(str::trim).filter(|s| !s.is_empty()) else {
            return if want.is_empty() {
                Freshness::Fresh
            } else {
                Freshness::Stale
            };
        };
        let parsed: Vec<serde_json::Value> = match serde_json::from_str(installed) {
            Ok(list) => list,
            Err(e) => {
                return Freshness::Unverifiable {
                    reason: format!(
                        "the installed managed MCP server list is not a JSON array: {e}"
                    ),
                };
            },
        };
        let mut have: Vec<&str> = parsed
            .iter()
            .filter_map(|entry| entry.get("name").and_then(serde_json::Value::as_str))
            .collect();
        have.sort_unstable();
        have.dedup();
        if have == want {
            Freshness::Fresh
        } else {
            Freshness::Stale
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
