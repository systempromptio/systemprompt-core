//! The update affordance the GUI shows, and how a check result moves it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{StagedUpdate, UpdateStatus};

/// What the GUI shows for the update affordance. Carried on the state snapshot
/// so it rides the existing `state.changed` event.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(tag = "phase", rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum UpdateUiState {
    #[default]
    Unknown,
    Current,
    Available {
        version: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        notes_url: Option<String>,
    },
    Downloading {
        version: String,
        percent: u8,
    },
    Ready {
        version: String,
    },
    Failed {
        message: String,
    },
}

impl UpdateUiState {
    #[must_use]
    pub const fn tone(&self) -> crate::verdict::Tone {
        use crate::verdict::Tone;
        match self {
            Self::Unknown => Tone::Unknown,
            Self::Current => Tone::Ok,
            Self::Available { .. } | Self::Ready { .. } => Tone::Warn,
            Self::Downloading { .. } => Tone::Probing,
            Self::Failed { .. } => Tone::Err,
        }
    }

    #[must_use]
    pub const fn can_install(&self) -> bool {
        matches!(self, Self::Available { .. })
    }

    #[must_use]
    pub const fn can_restart(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    #[must_use]
    pub const fn in_progress(&self) -> bool {
        matches!(self, Self::Downloading { .. })
    }

    #[must_use]
    pub fn version(&self) -> Option<&str> {
        match self {
            Self::Available { version, .. }
            | Self::Downloading { version, .. }
            | Self::Ready { version } => Some(version),
            Self::Unknown | Self::Current | Self::Failed { .. } => None,
        }
    }
}

impl From<&UpdateStatus> for UpdateUiState {
    fn from(status: &UpdateStatus) -> Self {
        match status {
            UpdateStatus::Current { .. } => Self::Current,
            UpdateStatus::Available { version, notes_url } => Self::Available {
                version: version.clone(),
                notes_url: notes_url.clone(),
            },
            UpdateStatus::Staged { version } => Self::Ready {
                version: version.clone(),
            },
        }
    }
}

impl UpdateUiState {
    #[must_use]
    pub fn seeded(staged: Option<&StagedUpdate>) -> Self {
        staged.map_or(Self::Unknown, |staged| Self::Ready {
            version: staged.version.clone(),
        })
    }

    #[must_use]
    pub fn after_check(&self, checked: Self) -> Self {
        match (self, checked) {
            (Self::Downloading { .. }, _)
            | (Self::Ready { .. }, Self::Failed { .. } | Self::Unknown) => self.clone(),
            (_, checked) => checked,
        }
    }
}
