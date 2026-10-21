//! Unattended updates: the one place that decides whether this bridge updates
//! itself, and the one routine that does it.
//!
//! Two triggers start an attempt. The GUI's event loop raises
//! [`Trigger::Schedule`] every [`SCHEDULE_INTERVAL`] and [`Trigger::Wake`]
//! after the machine sleeps, looking for any newer release. The proxy heartbeat
//! raises [`Trigger::Unsupported`] when the gateway reports this bridge below
//! its floor, which is the only trigger a headless (Linux) bridge has. Every
//! trigger passes the same [`begin`] gate — org policy, due time, single
//! flight — and the same [`Permit`] steps, so the platforms differ only in
//! who raises the trigger, never in what an attempt does.
//!
//! An attempt stages: it verifies the target is writable before any network
//! call, downloads and verifies the artifact, and swaps it over the install.
//! It never relaunches; the next natural launch runs the new version
//! ([`AutoUpdatePolicy::Staged`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
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

/// Why [`begin`] declined to start an attempt.
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

struct Gate {
    in_flight: bool,
    last_attempt: Option<Instant>,
}

static GATE: Mutex<Gate> = Mutex::new(Gate {
    in_flight: false,
    last_attempt: None,
});

/// Claims the single automatic-update slot for `trigger`.
///
/// The gate is tested before the policy read, so the GUI's one-second tick
/// does not touch the last-sync sentinel until an attempt is due, and again
/// after it, because the read runs without the lock held.
pub fn begin(trigger: Trigger) -> Result<Permit, Skipped> {
    lock_gate().admits(trigger)?;
    let decision = auto_update_policy();
    if !decision.stages() {
        return Err(Skipped::Policy(decision));
    }
    let mut gate = lock_gate();
    gate.admits(trigger)?;
    gate.in_flight = true;
    gate.last_attempt = Some(Instant::now());
    drop(gate);
    Ok(Permit { trigger })
}

fn lock_gate() -> std::sync::MutexGuard<'static, Gate> {
    GATE.lock().unwrap_or_else(PoisonError::into_inner)
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
}

impl Drop for Permit {
    fn drop(&mut self) {
        GATE.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .in_flight = false;
    }
}

impl Permit {
    #[must_use]
    pub const fn trigger(&self) -> Trigger {
        self.trigger
    }

    /// The newer release to stage, or `None` when this bridge is current.
    ///
    /// Writability is checked first: an install that cannot be replaced (a
    /// macOS app run from its disk image or App Translocation, a root-owned
    /// binary) fails here with the platform's fix, before any download.
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

/// A whole attempt with no progress reporting, for callers without a UI.
pub async fn run(
    trigger: Trigger,
    client: &GatewayClient,
    bearer: &BearerToken,
) -> Result<Outcome, AutomaticError> {
    let permit = begin(trigger)?;
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
