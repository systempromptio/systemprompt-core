//! The self-update policy a bridge reads from the signed manifest.
//!
//! The services manifest sets it under `bridge_policy.auto_update`; the
//! gateway signs it into the bridge manifest and the bridge obeys it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// Whether a bridge updates itself, and how far it is allowed to go on its own.
///
/// `Staged` downloads, verifies and swaps the on-disk binary but never restarts
/// the running process: the next natural launch runs the new version. There is
/// deliberately no variant that restarts unattended — the fleet-wide brake for
/// a bad release is `pinned_version` on the release feed, not a client toggle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoUpdatePolicy {
    Disabled,
    #[default]
    Staged,
}

impl AutoUpdatePolicy {
    #[must_use]
    pub const fn stages(self) -> bool {
        matches!(self, Self::Staged)
    }
}
