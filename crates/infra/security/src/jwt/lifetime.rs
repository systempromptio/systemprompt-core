//! The lifetime bound of every token signed with the deployment key.
//!
//! Every RS256 token this deployment mints is bounded either by a profile
//! value or by [`MAX_TOKEN_LIFETIME`], which every minting path enforces.
//! [`signed_token_lifetimes`] lists each kind with its bound, read from the
//! same profile fields the minting paths read, so a key rotation can keep the
//! retired key verifying for as long as any token it signed can still be
//! valid.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Duration;
use systemprompt_manifest::profile::SecurityConfig;

pub const MAX_TOKEN_LIFETIME: Duration = Duration::days(365);

#[must_use]
pub fn within_max_lifetime(ttl: Duration) -> bool {
    ttl > Duration::zero() && ttl <= MAX_TOKEN_LIFETIME
}

/// A kind of token signed with the deployment key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignedTokenKind {
    AccessToken,
    IdJag,
    CallerLifetimeUserToken,
    CliSessionToken,
    ExtensionSignedClaims,
}

impl SignedTokenKind {
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::AccessToken => {
                "access tokens (OAuth code/refresh, client-credentials, token exchange, SSO and \
                 anonymous sessions, A2A/MCP session tokens)"
            },
            Self::IdJag => "ID-JAG assertions",
            Self::CallerLifetimeUserToken => {
                "plugin and hook tokens, bridge and device tokens, agent-run MCP tokens"
            },
            Self::CliSessionToken => "CLI session and admin tokens",
            Self::ExtensionSignedClaims => "extension-signed claims",
        }
    }

    #[must_use]
    pub const fn configured_by(self) -> &'static str {
        match self {
            Self::AccessToken => "security.jwt_access_token_expiration",
            Self::IdJag => "security.id_jag_ttl_secs",
            Self::CallerLifetimeUserToken | Self::CliSessionToken | Self::ExtensionSignedClaims => {
                "MAX_TOKEN_LIFETIME (365 days)"
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignedTokenLifetime {
    pub kind: SignedTokenKind,
    pub max: Duration,
}

#[must_use]
pub fn signed_token_lifetimes(security: &SecurityConfig) -> Vec<SignedTokenLifetime> {
    let configured = |secs: i64| Duration::seconds(secs).min(MAX_TOKEN_LIFETIME);
    vec![
        SignedTokenLifetime {
            kind: SignedTokenKind::AccessToken,
            max: configured(security.access_token_expiration),
        },
        SignedTokenLifetime {
            kind: SignedTokenKind::IdJag,
            max: configured(security.id_jag_ttl_secs),
        },
        SignedTokenLifetime {
            kind: SignedTokenKind::CallerLifetimeUserToken,
            max: MAX_TOKEN_LIFETIME,
        },
        SignedTokenLifetime {
            kind: SignedTokenKind::CliSessionToken,
            max: MAX_TOKEN_LIFETIME,
        },
        SignedTokenLifetime {
            kind: SignedTokenKind::ExtensionSignedClaims,
            max: MAX_TOKEN_LIFETIME,
        },
    ]
}

#[must_use]
pub fn required_overlap(lifetimes: &[SignedTokenLifetime]) -> Duration {
    lifetimes
        .iter()
        .map(|lifetime| lifetime.max)
        .max()
        .unwrap_or(MAX_TOKEN_LIFETIME)
}

#[must_use]
pub fn cut_off_by(
    lifetimes: &[SignedTokenLifetime],
    overlap: Duration,
) -> Vec<SignedTokenLifetime> {
    lifetimes
        .iter()
        .filter(|lifetime| lifetime.max > overlap)
        .copied()
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum RotationOverlapError {
    #[error("the rotation overlap must be positive (got {seconds}s)")]
    NotPositive { seconds: i64 },

    #[error(
        "an overlap of {overlap_secs}s ends before these tokens signed with the replaced key \
         expire: {summary}. Pass --force to cut them off"
    )]
    CutsOff {
        overlap_secs: i64,
        cut_off: Vec<SignedTokenLifetime>,
        summary: String,
    },
}

pub fn rotation_overlap(
    security: &SecurityConfig,
    requested: Option<Duration>,
    force: bool,
) -> Result<Duration, RotationOverlapError> {
    let lifetimes = signed_token_lifetimes(security);
    let Some(overlap) = requested else {
        return Ok(required_overlap(&lifetimes));
    };
    if overlap <= Duration::zero() {
        return Err(RotationOverlapError::NotPositive {
            seconds: overlap.num_seconds(),
        });
    }
    let cut_off = cut_off_by(&lifetimes, overlap);
    if cut_off.is_empty() || force {
        return Ok(overlap);
    }
    let summary = cut_off
        .iter()
        .map(|lifetime| {
            format!(
                "{} (up to {}s, {})",
                lifetime.kind.description(),
                lifetime.max.num_seconds(),
                lifetime.kind.configured_by()
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(RotationOverlapError::CutsOff {
        overlap_secs: overlap.num_seconds(),
        cut_off,
        summary,
    })
}
