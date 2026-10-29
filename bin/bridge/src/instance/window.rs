//! The release window across every gateway profile on this install.
//!
//! Sync is gated on the active profile's window alone. An update is not: the
//! binary is shared by every profile, so a candidate release must sit inside
//! the intersection of all windows the profiles' verified manifests delivered.
//! Windows that do not overlap are a conflict, and nothing is staged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_models::bridge::instance::{BridgeVersionWindow, WindowStatus};

use crate::config::gateways::{self, GatewayProfileId, ProfileError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombinedWindow {
    Open,
    Window(BridgeVersionWindow),
    Conflict(Vec<(GatewayProfileId, BridgeVersionWindow)>),
}

#[must_use]
pub fn combine(windows: &[(GatewayProfileId, BridgeVersionWindow)]) -> CombinedWindow {
    let mut combined = BridgeVersionWindow::default();
    for (_, window) in windows {
        combined = combined.intersect(window);
    }
    if combined.is_empty() {
        CombinedWindow::Conflict(windows.to_vec())
    } else if combined.is_open() {
        CombinedWindow::Open
    } else {
        CombinedWindow::Window(combined)
    }
}

pub fn combined() -> Result<CombinedWindow, ProfileError> {
    Ok(combine(&gateways::delivered_windows()?))
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CandidateRefusal {
    #[error("release {candidate} is outside the window every gateway profile admits ({window})")]
    Outside { candidate: String, window: String },
    #[error("the gateway profiles admit no common release: {0}")]
    Conflict(String),
}

pub fn admit_candidate(
    combined: &CombinedWindow,
    candidate: &semver::Version,
) -> Result<(), CandidateRefusal> {
    match combined {
        CombinedWindow::Open => Ok(()),
        CombinedWindow::Window(window) if window.admits(candidate) => Ok(()),
        CombinedWindow::Window(window) => Err(CandidateRefusal::Outside {
            candidate: candidate.to_string(),
            window: describe(window),
        }),
        CombinedWindow::Conflict(windows) => {
            Err(CandidateRefusal::Conflict(conflict_text(windows)))
        },
    }
}

fn conflict_text(windows: &[(GatewayProfileId, BridgeVersionWindow)]) -> String {
    windows
        .iter()
        .map(|(id, w)| format!("{id} {}", describe(w)))
        .collect::<Vec<_>>()
        .join(", ")
}

#[must_use]
pub fn describe(window: &BridgeVersionWindow) -> String {
    let min = window
        .min
        .as_ref()
        .map_or_else(|| "any".to_owned(), ToString::to_string);
    let max = window
        .max
        .as_ref()
        .map_or_else(|| "any".to_owned(), ToString::to_string);
    format!("[{min}, {max}]")
}

/// The active profile's window and the cross-profile verdict, for the GUI and
/// `doctor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReleaseWindowReport {
    pub status: WindowStatus,
    pub min: Option<String>,
    pub max: Option<String>,
    pub enforce_max: bool,
    pub conflict: Option<String>,
}

impl Default for ReleaseWindowReport {
    fn default() -> Self {
        Self {
            status: WindowStatus::Unknown,
            min: None,
            max: None,
            enforce_max: false,
            conflict: None,
        }
    }
}

pub fn report() -> Result<ReleaseWindowReport, ProfileError> {
    let profile = gateways::active()?;
    let window = gateways::read_delivered(&profile)?.and_then(|d| d.window);
    let conflict = match combined()? {
        CombinedWindow::Conflict(windows) => Some(conflict_text(&windows)),
        CombinedWindow::Open | CombinedWindow::Window(_) => None,
    };
    let local = crate::brand::brand().version;
    Ok(window.map_or_else(
        || ReleaseWindowReport {
            conflict: conflict.clone(),
            ..ReleaseWindowReport::default()
        },
        |w| ReleaseWindowReport {
            status: w.status(local),
            min: w.min.as_ref().map(ToString::to_string),
            max: w.max.as_ref().map(ToString::to_string),
            enforce_max: w.enforce_max,
            conflict: conflict.clone(),
        },
    ))
}
