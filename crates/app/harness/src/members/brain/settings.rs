//! The person's own preferences for the brain, kept by the shell as the
//! extension's settings object.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// The brain preferences: `recall` off asks the instance's gateway not to
/// recall team memory into this app's model calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrainSettings {
    #[serde(default = "recall_by_default")]
    pub recall: bool,
}

impl Default for BrainSettings {
    fn default() -> Self {
        Self {
            recall: recall_by_default(),
        }
    }
}

const fn recall_by_default() -> bool {
    true
}
