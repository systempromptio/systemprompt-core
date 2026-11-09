//! Whether this install may stage releases on its own, as delivered by the
//! gateway's last synced manifest.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gateway::manifest::AutoUpdatePolicy;

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
