//! The codes the snapshot-wide verdicts carry: gateway, identity, session,
//! health and the one-dot summary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

/// The gateway probe's outcome, without its payload.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
#[serde(rename_all = "kebab-case")]
pub enum GatewayCode {
    Unknown,
    Probing,
    Reachable,
    Unreachable,
}

/// Whether this bridge is signed in, and if not, why.
///
/// `TokenRejected` and `Verifying` used to be the same fact read two ways:
/// Setup called a stored-but-unverified token rejected, Status called it
/// "verifying", and both were looking at `pat_present && !verified`. The
/// gateway probe is the tie-breaker — unverified against a reachable gateway
/// is a rejection; unverified while the probe is still out is not.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
#[serde(rename_all = "kebab-case")]
pub enum IdentityCode {
    GatewayUnreachable,
    Verifying,
    SignedIn,
    TokenRejected,
    SignedOut,
}

/// The single dot the footer and sync pill show.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
#[serde(rename_all = "kebab-case")]
pub enum OverallCode {
    Syncing,
    Offline,
    Synced,
    Ready,
    NeedsSignIn,
}

/// The cached session token's state.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
#[serde(rename_all = "kebab-case")]
pub enum TokenCode {
    Missing,
    Expiring,
    Valid,
}

/// The health board's badge: validation, providers, plugins and last sync,
/// folded.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
#[serde(rename_all = "kebab-case")]
pub enum HealthCode {
    NotChecked,
    Healthy,
    Attention,
    Failing,
}
