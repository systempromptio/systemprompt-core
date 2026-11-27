//! UI-facing state: what the shell and every window may see of accounts and
//! sync. Nothing here holds a credential.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::AccountId;

/// A manifest key fingerprint as shown and compared: `sha256:<64 hex>`
/// (`names::key_fingerprint`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeyFingerprint(String);

impl KeyFingerprint {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn of_key(raw_key: &[u8]) -> Self {
        Self(crate::names::key_fingerprint(raw_key))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for KeyFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether the harness holds a credential for an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccountStatus {
    SignedIn,
    SignedOut,
}

/// One known instance as the UI sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountView {
    pub id: AccountId,
    pub label: String,
    pub url: String,
    pub user: Option<String>,
    pub org: Option<String>,
    pub status: AccountStatus,
    pub active: bool,
    pub fingerprint: Option<KeyFingerprint>,
    pub pinned: Option<KeyFingerprint>,
    pub trusted: bool,
}

/// A served manifest key that differs from the pinned one (or the first one,
/// with nothing pinned), offered to the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyOffer {
    pub pinned: Option<KeyFingerprint>,
    pub offered: KeyFingerprint,
}

/// What the harness holds of the active instance after a sync.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncCounts {
    pub skills: u32,
    pub agents: u32,
    pub mcp: u32,
    pub rules: u32,
    pub warnings: Vec<String>,
}

/// Why the last sync of the active instance failed.
///
/// `unreachable` (no answer, a timeout or a 5xx), `rate-limited`,
/// `refused` (the instance refused the credential), `key-changed` (the instance
/// signs with a key the user has not trusted; sync stops until it is),
/// `invalid` (a manifest that does not verify, decode, belong to the user, or
/// is not newer than the last one), `insecure` (the OS secure store does not
/// open the PAT right now) and `service` (the harness's own apply step failed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SyncErrorKind {
    Unreachable,
    RateLimited,
    Refused,
    KeyChanged,
    Invalid,
    Insecure,
    Service,
}

/// The live sync of the active instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum SyncStatus {
    Idle,
    Syncing {
        account: AccountId,
    },
    Synced {
        account: AccountId,
        at: DateTime<Utc>,
        counts: SyncCounts,
    },
    Error {
        account: AccountId,
        kind: SyncErrorKind,
        at: DateTime<Utc>,
    },
}

impl SyncStatus {
    #[must_use]
    pub const fn account(&self) -> Option<&AccountId> {
        match self {
            Self::Idle => None,
            Self::Syncing { account }
            | Self::Synced { account, .. }
            | Self::Error { account, .. } => Some(account),
        }
    }
}

/// Why a sign-in failed, without the client's error (which could quote a
/// credential).
///
/// `unreachable` (no answer),
/// `refused` (the token or code was rejected), `rate-limited`, `incompatible`
/// (not a supported instance), `insecure` (no OS secure storage) and `failed`
/// (anything else).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SignInFailure {
    Unreachable,
    Refused,
    RateLimited,
    Incompatible,
    Insecure,
    Failed,
}

/// How a sign-in ended. `denied` is the user pressing Deny on the instance's
/// consent page; `expired` is nothing coming back within the sign-in window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SignInOutcome {
    SignedIn,
    Cancelled,
    Denied,
    Expired,
    Failed(SignInFailure),
}

/// What an instance's first manifest holds, shown before or right after
/// sign-in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSummary {
    pub skills: u32,
    pub agents: u32,
    pub mcp: u32,
    pub role: Option<String>,
}
