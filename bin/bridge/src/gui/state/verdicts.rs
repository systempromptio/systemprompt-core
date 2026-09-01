//! Verdicts over the whole snapshot — gateway, identity, session, health, and
//! the one-dot summary — computed here so no two panes reach different answers
//! from the same state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::types::{AppStateSnapshot, GatewayStatus};
use crate::verdict::{Tone, Verdict};
pub use crate::wire::codes::{GatewayCode, HealthCode, IdentityCode, OverallCode, TokenCode};

const TOKEN_EXPIRY_WARN_SECONDS: u64 = 600;

impl GatewayStatus {
    #[must_use]
    pub const fn code(&self) -> GatewayCode {
        match self {
            Self::Unknown => GatewayCode::Unknown,
            Self::Probing => GatewayCode::Probing,
            Self::Reachable { .. } => GatewayCode::Reachable,
            Self::Unreachable { .. } => GatewayCode::Unreachable,
        }
    }

    #[must_use]
    pub const fn tone(&self) -> Tone {
        match self {
            Self::Unknown => Tone::Unknown,
            Self::Probing => Tone::Probing,
            Self::Reachable { .. } => Tone::Ok,
            Self::Unreachable { .. } => Tone::Err,
        }
    }

    #[must_use]
    pub const fn verdict(&self) -> Verdict<GatewayCode> {
        Verdict::new(self.tone(), self.code())
    }

    #[must_use]
    pub const fn settled(&self) -> bool {
        matches!(self, Self::Reachable { .. } | Self::Unreachable { .. })
    }
}

impl AppStateSnapshot {
    #[must_use]
    pub fn identity_verdict(&self) -> Verdict<IdentityCode> {
        let verified = self
            .verified_identity
            .as_ref()
            .is_some_and(|id| id.user_id.is_some() || id.email.is_some());
        match &self.gateway_status {
            GatewayStatus::Unreachable { .. } => {
                Verdict::new(Tone::Unknown, IdentityCode::GatewayUnreachable)
            },
            GatewayStatus::Reachable { .. } if verified => {
                Verdict::new(Tone::Ok, IdentityCode::SignedIn)
            },
            GatewayStatus::Reachable { .. } if self.pat_present => {
                Verdict::new(Tone::Err, IdentityCode::TokenRejected)
            },
            GatewayStatus::Reachable { .. } => Verdict::new(Tone::Warn, IdentityCode::SignedOut),
            GatewayStatus::Unknown | GatewayStatus::Probing if self.pat_present => {
                Verdict::new(Tone::Probing, IdentityCode::Verifying)
            },
            GatewayStatus::Unknown | GatewayStatus::Probing => {
                Verdict::new(Tone::Warn, IdentityCode::SignedOut)
            },
        }
    }

    #[must_use]
    pub const fn overall_verdict(&self) -> Verdict<OverallCode> {
        if self.sync_in_flight {
            return Verdict::new(Tone::Probing, OverallCode::Syncing);
        }
        if matches!(self.gateway_status, GatewayStatus::Unreachable { .. }) {
            return Verdict::new(Tone::Err, OverallCode::Offline);
        }
        if self.signed_in() {
            return if self.last_sync_summary.is_some() {
                Verdict::new(Tone::Ok, OverallCode::Synced)
            } else {
                Verdict::new(Tone::Ok, OverallCode::Ready)
            };
        }
        Verdict::new(Tone::Warn, OverallCode::NeedsSignIn)
    }

    #[must_use]
    pub const fn token_verdict(&self) -> Verdict<TokenCode> {
        match &self.cached_token {
            None => Verdict::new(Tone::Err, TokenCode::Missing),
            Some(t) if t.ttl_seconds < TOKEN_EXPIRY_WARN_SECONDS => {
                Verdict::new(Tone::Warn, TokenCode::Expiring)
            },
            Some(_) => Verdict::new(Tone::Ok, TokenCode::Valid),
        }
    }

    // Why: the health board folds five sources, not just `validate`. Each
    // synthetic row's tone is fixed here and mirrored by the row the front end
    // draws; the *badge* is this fold, so the board cannot contradict it.
    #[must_use]
    pub fn health_verdict(&self) -> Verdict<HealthCode> {
        let mut tones: Vec<Tone> = Vec::new();
        if let Some(report) = &self.last_validation {
            tones.push(report.verdict().tone);
        }
        if self.provider_health.iter().any(|p| !p.configured) {
            tones.push(Tone::Warn);
        }
        if self.malformed_plugin_count.is_some_and(|n| n > 0) {
            tones.push(Tone::Err);
        }
        if let Some(sync) = &self.last_sync_report {
            if !sync.host_failures.is_empty() {
                tones.push(Tone::Err);
            }
            if !sync.diagnostics.is_empty() {
                tones.push(Tone::Warn);
            }
        }
        if tones.is_empty() {
            return Verdict::new(Tone::Unknown, HealthCode::NotChecked);
        }
        let tone = Tone::fold(tones, Tone::Ok);
        let code = match tone {
            Tone::Err => HealthCode::Failing,
            Tone::Warn => HealthCode::Attention,
            Tone::Ok | Tone::Unknown | Tone::Probing => HealthCode::Healthy,
        };
        Verdict::new(tone, code)
    }

    // Why: `probing` while the first pass is out and `warn` when nothing is
    // registered — an empty list is not a healthy one.
    #[must_use]
    pub fn mcp_auth_tone(&self) -> Tone {
        if self.mcp_auth.is_empty() {
            return if self.mcp_auth_probe_in_flight {
                Tone::Probing
            } else {
                Tone::Warn
            };
        }
        Tone::fold(self.mcp_auth.iter().map(|s| s.state.tone()), Tone::Unknown)
    }

    #[must_use]
    pub fn cloud_tone(&self) -> Tone {
        self.gateway_status
            .tone()
            .worst(self.identity_verdict().tone)
    }
}
