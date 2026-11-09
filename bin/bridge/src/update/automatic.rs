//! Unattended updates: the one place that decides whether this bridge updates
//! itself, and the one routine that does it.
//!
//! Two triggers start an attempt. The GUI's event loop raises
//! [`Trigger::Schedule`] every [`SCHEDULE_INTERVAL`] and [`Trigger::Wake`]
//! after the machine sleeps, looking for any newer release. The proxy heartbeat
//! raises [`Trigger::Unsupported`] when the gateway reports this bridge below
//! its floor, which is the only trigger a headless (Linux) bridge has. Every
//! trigger passes the same [`UpdateGate::begin`] gate — org policy, due time,
//! single flight — and the same [`Permit`] steps, so the platforms differ only
//! in who raises the trigger, never in what an attempt does.
//!
//! An attempt stages: it verifies the target is writable before any network
//! call, downloads and verifies the artifact, and swaps it over the install.
//! It never relaunches: the update finishes when the bridge restarts, or when a
//! newer build is launched and takes over from the running one
//! ([`crate::gateway::manifest::AutoUpdatePolicy::Staged`]). A release
//! already staged is reported as staged without another download.
//!
//! [`UpdateGate::begin`] tests the gate before the policy read, so the GUI's
//! one-second tick does not touch the last-sync sentinel until an attempt is
//! due, and again after it, because the read runs without the lock held.
//! `Permit::check` checks writability first: an install that cannot be
//! replaced (a macOS app run from its disk image or App Translocation, a
//! root-owned binary) fails there with the platform's fix, before any
//! download. [`run`] is a whole attempt with no progress reporting, for
//! callers without a UI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use super::policy::{AutoUpdateDecision, auto_update_policy};
use super::{DownloadProgress, UpdateError, UpdateStatus, apply, check, install};
use crate::gateway::GatewayClient;
use crate::gateway::types::ReleaseManifest;
use crate::ids::BearerToken;

pub const SCHEDULE_INTERVAL: Duration = Duration::from_hours(6);

pub const UNSUPPORTED_RETRY: Duration = Duration::from_mins(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Schedule,
    Wake,
    Unsupported,
}

impl Trigger {
    const fn min_gap(self) -> Option<Duration> {
        match self {
            Self::Schedule => Some(SCHEDULE_INTERVAL),
            Self::Wake => None,
            Self::Unsupported => Some(UNSUPPORTED_RETRY),
        }
    }
}

/// Why [`UpdateGate::begin`] declined to start an attempt.
#[derive(Debug, thiserror::Error)]
pub enum Skipped {
    #[error("automatic updates are off: {}", .0.describe())]
    Policy(AutoUpdateDecision),
    #[error("an automatic update was attempted recently")]
    NotDue,
    #[error("an automatic update is already running")]
    InFlight,
}

#[derive(Debug, thiserror::Error)]
pub enum AutomaticError {
    #[error(transparent)]
    Skipped(#[from] Skipped),
    #[error(transparent)]
    Update(#[from] UpdateError),
}

/// What [`Permit::check`] found: nothing to do, a release already staged, or
/// a release to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checked {
    Current,
    AlreadyStaged { version: String },
    Install(ReleaseManifest),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Current,
    Staged { version: String, path: PathBuf },
}

#[derive(Debug)]
struct Gate {
    in_flight: bool,
    last_attempt: Option<Instant>,
}

/// The single automatic-update slot of this process, built once by
/// `BridgeContext::start` and shared by every trigger.
#[derive(Debug, Clone)]
pub struct UpdateGate(Arc<Mutex<Gate>>);

impl Default for UpdateGate {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(Gate {
            in_flight: false,
            last_attempt: None,
        })))
    }
}

impl UpdateGate {
    pub fn begin(&self, trigger: Trigger) -> Result<Permit, Skipped> {
        self.lock().admits(trigger)?;
        let decision = auto_update_policy();
        if !decision.stages() {
            return Err(Skipped::Policy(decision));
        }
        let mut gate = self.lock();
        gate.admits(trigger)?;
        gate.in_flight = true;
        gate.last_attempt = Some(Instant::now());
        drop(gate);
        Ok(Permit {
            trigger,
            gate: self.clone(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Gate> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Gate {
    fn admits(&self, trigger: Trigger) -> Result<(), Skipped> {
        if self.in_flight {
            return Err(Skipped::InFlight);
        }
        match (trigger.min_gap(), self.last_attempt) {
            (Some(gap), Some(at)) if at.elapsed() < gap => Err(Skipped::NotDue),
            _ => Ok(()),
        }
    }
}

/// The claimed slot; dropping it frees the slot for the next trigger.
#[derive(Debug)]
pub struct Permit {
    trigger: Trigger,
    gate: UpdateGate,
}

impl Drop for Permit {
    fn drop(&mut self) {
        self.gate.lock().in_flight = false;
    }
}

impl Permit {
    #[must_use]
    pub const fn trigger(&self) -> Trigger {
        self.trigger
    }

    pub async fn check(
        &self,
        client: &GatewayClient,
        bearer: &BearerToken,
    ) -> Result<Checked, UpdateError> {
        install::preflight()?;
        let (status, manifest) = check(client, bearer).await?;
        match status {
            UpdateStatus::Staged { version } => Ok(Checked::AlreadyStaged { version }),
            UpdateStatus::Available { ref version, .. } if is_downgrade(version) => {
                if downgrade_enforced() {
                    Ok(Checked::Install(manifest))
                } else {
                    tracing::warn!(
                        local = %crate::brand::brand().version,
                        offered = %version,
                        "this release is newer than the instance admits; install the offered \
                         release from the update panel"
                    );
                    Ok(Checked::Current)
                }
            },
            UpdateStatus::Available { .. } => Ok(Checked::Install(manifest)),
            UpdateStatus::Current { .. } => {
                if self.trigger == Trigger::Unsupported {
                    tracing::warn!(
                        local = %crate::brand::brand().version,
                        "the gateway reports this bridge as unsupported but offers no newer release",
                    );
                }
                Ok(Checked::Current)
            },
        }
    }

    pub async fn stage(
        self,
        client: &GatewayClient,
        bearer: &BearerToken,
        manifest: &ReleaseManifest,
        on_progress: &(dyn Fn(DownloadProgress) + Send + Sync),
    ) -> Result<PathBuf, UpdateError> {
        let path = apply(client, bearer, manifest, on_progress).await?;
        tracing::info!(
            version = %manifest.version,
            path = %path.display(),
            trigger = ?self.trigger,
            "automatic update staged; it finishes when the bridge restarts or a newer build is launched",
        );
        Ok(path)
    }
}

fn is_downgrade(offered: &str) -> bool {
    match (
        semver::Version::parse(offered),
        semver::Version::parse(crate::brand::brand().version),
    ) {
        (Ok(offered), Ok(local)) => offered.cmp_precedence(&local).is_lt(),
        _ => false,
    }
}

fn downgrade_enforced() -> bool {
    match crate::instance::window::report() {
        Ok(report) => report.enforce_max,
        Err(error) => {
            tracing::warn!(%error, "release window unreadable; an older release is not staged");
            false
        },
    }
}

pub async fn run(
    gate: &UpdateGate,
    trigger: Trigger,
    client: &GatewayClient,
    bearer: &BearerToken,
) -> Result<Outcome, AutomaticError> {
    let permit = gate.begin(trigger)?;
    let manifest = match permit.check(client, bearer).await? {
        Checked::Current => return Ok(Outcome::Current),
        Checked::AlreadyStaged { version } => {
            return Ok(Outcome::Staged {
                version,
                path: super::installed_path()?,
            });
        },
        Checked::Install(manifest) => manifest,
    };
    let path = permit.stage(client, bearer, &manifest, &|_| {}).await?;
    Ok(Outcome::Staged {
        version: manifest.version,
        path,
    })
}
