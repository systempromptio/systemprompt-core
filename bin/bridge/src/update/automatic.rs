//! Unattended updates: the one place that decides whether this bridge updates
//! itself, and the one routine that does it.
//!
//! Two triggers start an attempt. The GUI's event loop raises
//! [`Trigger::Schedule`] every [`SCHEDULE_INTERVAL`] and [`Trigger::Wake`]
//! after the machine sleeps, looking for any newer release. The proxy heartbeat
//! raises [`Trigger::Unsupported`] when the gateway reports this bridge below
//! its floor, which is the only trigger a headless (Linux) bridge has. Every
//! trigger passes the same [`UpdateGate::begin`] gate — org policy, due time, single
//! flight — and the same [`Permit`] steps, so the platforms differ only in
//! who raises the trigger, never in what an attempt does.
//!
//! An attempt stages: it verifies the target is writable before any network
//! call, downloads and verifies the artifact, and swaps it over the install.
//! It never relaunches; the next natural launch runs the new version
//! ([`AutoUpdatePolicy::Staged`]).
//!
//! [`UpdateGate::begin`] tests the gate before the policy read, so the GUI's one-second
//! tick does not touch the last-sync sentinel until an attempt is due, and
//! again after it, because the read runs without the lock held.
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

use super::{DownloadProgress, UpdateError, UpdateStatus, apply, check, install};
use crate::gateway::GatewayClient;
use crate::gateway::manifest::AutoUpdatePolicy;
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
    ) -> Result<Option<ReleaseManifest>, UpdateError> {
        install::preflight()?;
        let (status, manifest) = check(client, bearer).await?;
        match status {
            UpdateStatus::Available { .. } => Ok(Some(manifest)),
            UpdateStatus::Current { .. } => {
                if self.trigger == Trigger::Unsupported {
                    tracing::warn!(
                        local = %crate::brand::brand().version,
                        "the gateway reports this bridge as unsupported but offers no newer release",
                    );
                }
                Ok(None)
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
            "automatic update staged; the next launch runs it",
        );
        Ok(path)
    }
}

pub async fn run(
    gate: &UpdateGate,
    trigger: Trigger,
    client: &GatewayClient,
    bearer: &BearerToken,
) -> Result<Outcome, AutomaticError> {
    let permit = gate.begin(trigger)?;
    let Some(manifest) = permit.check(client, bearer).await? else {
        return Ok(Outcome::Current);
    };
    let path = permit.stage(client, bearer, &manifest, &|_| {}).await?;
    Ok(Outcome::Staged {
        version: manifest.version,
        path,
    })
}

/// Whether this install may stage releases on its own.
///
/// Only a policy the configured gateway delivered enables it; a bridge that
/// never synced, or whose sentinel cannot be read, is `Withheld` and stages
/// nothing.
#[derive(Debug)]
pub enum AutoUpdateDecision {
    Delivered(AutoUpdatePolicy),
    NeverSynced,
    Withheld(crate::last_sync::ReplayStateError),
}

impl AutoUpdateDecision {
    #[must_use]
    pub const fn stages(&self) -> bool {
        matches!(self, Self::Delivered(policy) if policy.stages())
    }

    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Delivered(policy) => format!("{policy:?} (delivered by the gateway)"),
            Self::NeverSynced => "withheld (no manifest synced yet)".to_owned(),
            Self::Withheld(e) => format!("withheld (last-sync sentinel unreadable: {e})"),
        }
    }
}

// Why: org policy is the only source; there is no local preference to
// contradict it, and an unreadable sentinel must not fall back to the
// policy default that stages.
#[must_use]
pub fn auto_update_policy() -> AutoUpdateDecision {
    match crate::last_sync::last_synced_auto_update_policy() {
        Ok(Some(policy)) => AutoUpdateDecision::Delivered(policy),
        Ok(None) => AutoUpdateDecision::NeverSynced,
        Err(e) => AutoUpdateDecision::Withheld(e),
    }
}

#[must_use]
pub fn automatic_enabled() -> bool {
    auto_update_policy().stages()
}
