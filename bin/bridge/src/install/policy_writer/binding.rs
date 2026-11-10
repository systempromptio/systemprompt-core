//! What the SYSTEM-side writer trusts, and the decisions it takes from it.
//!
//! The machine policy is bound to one account: the one that ran the elevated
//! install. That install records the account's SID and the loopback port and
//! host token the machine policy points at in a [`MachineBinding`] only
//! SYSTEM and Administrators can write. A request is acted on only when the
//! account that owns the request file is that SID; the port and token come
//! from the binding, never from the request. The envelope a request carries
//! is refused when its `not_before` lies further in the future than the clock
//! skew allows, and admitted only when its version is newer than the last one
//! the writer accepted ([`AcceptedEnvelope`]). Its age is not bounded: the
//! monotonic version is what refuses an old envelope.
//!
//! Every decision here is a pure function so it runs on every platform; the
//! Windows shims only read the owner, the binding and the accepted record.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_models::bridge::manifest::{SignedManifest, SignedManifestEnvelope};

use super::{Loopback, PolicyWriterError};
use crate::gateway::manifest_version::ManifestVersion;
use crate::ids::{HostToken, Sha256Digest};

pub const BINDING_VERSION: u32 = 1;

/// A Windows account security identifier in its `S-1-…` string form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AccountSid(String);

impl AccountSid {
    pub fn try_new(sid: impl Into<String>) -> Result<Self, PolicyWriterError> {
        let sid = sid.into();
        let well_formed = sid.strip_prefix("S-1-").is_some_and(|rest| {
            !rest.is_empty()
                && rest
                    .split('-')
                    .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        });
        if well_formed {
            Ok(Self(sid))
        } else {
            Err(PolicyWriterError::InvalidSid(sid))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AccountSid {
    type Error = PolicyWriterError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<AccountSid> for String {
    fn from(value: AccountSid) -> Self {
        value.0
    }
}

impl std::fmt::Display for AccountSid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The account the machine policy belongs to and the loopback it points at,
/// recorded by the elevated install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineBinding {
    pub version: u32,
    pub installer_sid: AccountSid,
    pub loopback_port: u16,
    pub host_token: HostToken,
}

impl MachineBinding {
    #[must_use]
    pub fn new(installer_sid: AccountSid, loopback: Loopback) -> Self {
        Self {
            version: BINDING_VERSION,
            installer_sid,
            loopback_port: loopback.port,
            host_token: loopback.host_token,
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PolicyWriterError> {
        let binding: Self = serde_json::from_slice(bytes).map_err(|e| PolicyWriterError::Io {
            context: "decode the machine policy binding".to_owned(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
        })?;
        if binding.version != BINDING_VERSION {
            return Err(PolicyWriterError::BindingVersion {
                actual: binding.version,
            });
        }
        Ok(binding)
    }

    #[must_use]
    pub fn loopback(&self) -> Loopback {
        Loopback {
            port: self.loopback_port,
            host_token: self.host_token.clone(),
        }
    }
}

/// The newest manifest version the writer has accepted, and the digest of
/// the exact envelope that carried it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedEnvelope {
    pub manifest_version: ManifestVersion,
    pub envelope_digest: Sha256Digest,
}

/// How an envelope was admitted: one the writer has not seen, or the very
/// envelope it last accepted, presented again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Fresh,
    AlreadyAccepted,
}

/// What a write did to the machine hive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOutcome {
    Written,
    Reapplied,
    Unchanged,
}

impl ApplyOutcome {
    #[must_use]
    pub const fn of(admission: Admission, hive_differs: bool) -> Self {
        match (admission, hive_differs) {
            (Admission::Fresh, _) => Self::Written,
            (Admission::AlreadyAccepted, true) => Self::Reapplied,
            (Admission::AlreadyAccepted, false) => Self::Unchanged,
        }
    }
}

pub fn envelope_digest(
    envelope: &SignedManifestEnvelope,
) -> Result<Sha256Digest, PolicyWriterError> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(envelope.payload.as_bytes());
    hasher.update([0]);
    hasher.update(envelope.signature.as_str().as_bytes());
    Ok(Sha256Digest::try_new(crate::hash::hex_encode(
        &hasher.finalize(),
    ))?)
}

pub fn authorize(owner: &AccountSid, binding: &MachineBinding) -> Result<(), PolicyWriterError> {
    if *owner == binding.installer_sid {
        return Ok(());
    }
    Err(PolicyWriterError::NotBoundInstaller {
        owner: owner.clone(),
        bound: binding.installer_sid.clone(),
    })
}

pub fn require_bound_loopback(
    binding: &MachineBinding,
    current: &Loopback,
) -> Result<(), PolicyWriterError> {
    if binding.loopback() == *current {
        return Ok(());
    }
    Err(PolicyWriterError::BindingStale {
        bound_port: binding.loopback_port,
        current_port: current.port,
    })
}

// Why: the envelope the writer last accepted, presented again byte for
// byte, derives the same policy it already derived, so it is a re-apply and
// not a replay. Any other envelope must be newer than the accepted one and
// must not be dated beyond the future skew bound.
pub fn admit(
    manifest: &SignedManifest,
    digest: &Sha256Digest,
    last: Option<&AcceptedEnvelope>,
    now: DateTime<Utc>,
) -> Result<Admission, PolicyWriterError> {
    if let Some(last) = last
        && manifest.manifest_version == last.manifest_version
        && *digest == last.envelope_digest
    {
        return Ok(Admission::AlreadyAccepted);
    }
    if crate::sync::dated_beyond_skew(manifest.not_before, now) {
        return Err(PolicyWriterError::Skewed {
            not_before: manifest.not_before.to_rfc3339(),
            now: now.to_rfc3339(),
        });
    }
    if let Some(last) = last
        && manifest.manifest_version <= last.manifest_version
    {
        return Err(PolicyWriterError::Replayed {
            last: last.manifest_version.to_string(),
            incoming: manifest.manifest_version.to_string(),
        });
    }
    Ok(Admission::Fresh)
}
