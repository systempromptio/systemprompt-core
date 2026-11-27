//! Vendor identifiers carried by the association documents.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use crate::devices::{AppIdentifier, AppIdentifierError};

const APPLE_TEAM_ID_LEN: usize = 10;
const SHA256_BYTES: usize = 32;

/// A string that is not a valid association identifier of the named kind.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {kind} {value:?}: {reason}")]
pub struct AssociationValueError {
    pub kind: &'static str,
    pub value: String,
    pub reason: &'static str,
}

/// A string that is not a valid [`AppleAppId`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AppleAppIdError {
    #[error("an Apple app id is '<team id>.<bundle id>', got {0:?}")]
    Malformed(String),
    #[error(transparent)]
    TeamId(#[from] AssociationValueError),
    #[error(transparent)]
    BundleId(#[from] AppIdentifierError),
}

/// An Apple developer team identifier: ten ASCII uppercase letters or digits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AppleTeamId(String);

impl AppleTeamId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, AssociationValueError> {
        let value = value.into();
        let valid = value.len() == APPLE_TEAM_ID_LEN
            && value
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
        if valid {
            Ok(Self(value))
        } else {
            Err(AssociationValueError {
                kind: "Apple team id",
                value,
                reason: "must be ten ASCII uppercase letters or digits",
            })
        }
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// An Apple application identifier, `<team id>.<bundle id>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AppleAppId {
    team_id: AppleTeamId,
    bundle_id: AppIdentifier,
}

impl AppleAppId {
    #[must_use]
    pub const fn new(team_id: AppleTeamId, bundle_id: AppIdentifier) -> Self {
        Self { team_id, bundle_id }
    }

    #[must_use]
    pub const fn team_id(&self) -> &AppleTeamId {
        &self.team_id
    }

    #[must_use]
    pub const fn bundle_id(&self) -> &AppIdentifier {
        &self.bundle_id
    }
}

/// The SHA-256 fingerprint of an Android signing certificate: 32 bytes as
/// uppercase hex pairs joined by `:`. Lowercase input is normalised.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Sha256CertFingerprint(String);

impl Sha256CertFingerprint {
    pub fn try_new(value: impl AsRef<str>) -> Result<Self, AssociationValueError> {
        let value = value.as_ref();
        let pairs = value.split(':').collect::<Vec<_>>();
        let valid = pairs.len() == SHA256_BYTES
            && pairs
                .iter()
                .all(|pair| pair.len() == 2 && pair.bytes().all(|b| b.is_ascii_hexdigit()));
        if valid {
            Ok(Self(value.to_ascii_uppercase()))
        } else {
            Err(AssociationValueError {
                kind: "SHA-256 certificate fingerprint",
                value: value.to_owned(),
                reason: "must be 32 hex pairs joined by ':'",
            })
        }
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for AppleTeamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for AppleAppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.team_id, self.bundle_id)
    }
}

impl fmt::Display for Sha256CertFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for AppleTeamId {
    type Err = AssociationValueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_new(s)
    }
}

impl FromStr for AppleAppId {
    type Err = AppleAppIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (team, bundle) = s
            .split_once('.')
            .ok_or_else(|| AppleAppIdError::Malformed(s.to_owned()))?;
        Ok(Self {
            team_id: AppleTeamId::try_new(team)?,
            bundle_id: AppIdentifier::try_new(bundle)?,
        })
    }
}

impl FromStr for Sha256CertFingerprint {
    type Err = AssociationValueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_new(s)
    }
}

crate::notifications::string_form::string_serde!(AppleTeamId, "AppleTeamId");
crate::notifications::string_form::string_serde!(AppleAppId, "AppleAppId");
crate::notifications::string_form::string_serde!(Sha256CertFingerprint, "Sha256CertFingerprint");
