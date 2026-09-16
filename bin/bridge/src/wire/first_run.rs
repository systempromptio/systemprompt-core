//! First-run (setup) progress as the webview receives it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// How far the run has got. `Complete` and `Failed` are both terminal; the
/// difference is only what the wizard says, not whether the user may leave.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum FirstRunPhase {
    #[default]
    Idle,
    Probing,
    Installing,
    Syncing,
    Complete,
    Failed,
}

/// Where one host has got to. `Skipped` means the host app is not installed on
/// this machine, which is not a failure.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum StepStatus {
    #[default]
    Pending,
    Probing,
    Generating,
    Installing,
    Done,
    Failed,
    Skipped,
}

impl StepStatus {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Skipped)
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct FirstRunHostPayload<'a> {
    pub host_id: &'a str,
    pub display_name: &'a str,
    pub status: StepStatus,
    pub error: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct FirstRunPayload<'a> {
    pub active: bool,
    pub done: bool,
    pub phase: FirstRunPhase,
    pub sync: StepStatus,
    pub error: Option<&'a str>,
    pub hosts: Vec<FirstRunHostPayload<'a>>,
}
