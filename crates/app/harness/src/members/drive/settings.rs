//! The Drive's own preferences, kept by the shell under the extension id
//! `drive`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The Drive's preferences, read and written through
/// `SpContext::settings_for(&DriveExtension::ID)`.
///
/// `folder` (stored as the key `folder`) is the folder the user picked for
/// the local sync; each instance is mirrored into its own subfolder under it.
/// Without it the sync uses
/// [`DriveExtension::default_folder`](super::DriveExtension::default_folder),
/// and without either the sync stays off.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<PathBuf>,
}
