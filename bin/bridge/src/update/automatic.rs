//! Automatic staging of a required release, gated on the auto-update policy
//! the configured gateway delivered with its last manifest.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::ids::BearerToken;
use systemprompt_identifiers::ValidatedUrl;

use super::{UpdateStatus, apply, check, spawn_installed};
use crate::gateway::GatewayClient;
use crate::gateway::manifest::AutoUpdatePolicy;

pub async fn run_automatic(gateway: &ValidatedUrl, bearer: &BearerToken, http: &reqwest::Client) {
    let decision = auto_update_policy();
    if !decision.stages() {
        tracing::warn!(
            policy = %decision.describe(),
            "a newer bridge is required but automatic updates are not enabled by a delivered \
             policy; update manually",
        );
        return;
    }
    let client = GatewayClient::new(gateway.clone(), http.clone());
    let (status, manifest) = match check(&client, bearer).await {
        Ok(pair) => pair,
        Err(e) => {
            tracing::error!(error = %e, "automatic update: could not read the release manifest");
            return;
        },
    };
    if matches!(status, UpdateStatus::Current { .. }) {
        tracing::warn!(
            local = %crate::brand::brand().version,
            "the gateway reports this bridge as unsupported but offers no newer release",
        );
        return;
    }
    tracing::info!(version = %manifest.version, "automatic update: installing");
    let installed = match apply(&client, bearer, &manifest, &|_| {}).await {
        Ok(path) => path,
        Err(e) => {
            tracing::error!(error = %e, "automatic update: install failed");
            return;
        },
    };
    if let Err(e) = spawn_installed(&installed) {
        tracing::error!(error = %e, "automatic update: relaunch failed; update is staged on disk");
        return;
    }
    tracing::info!(version = %manifest.version, "automatic update: relaunched");
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
