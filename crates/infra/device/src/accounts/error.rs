//! The failure type of the accounts API outside the sign-in flows. No variant
//! carries a credential, a ciphertext or a value the store refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use crate::client::{ClientError, CredentialError};
use sp_protocol::ids::AccountId;
use crate::secure::SecureError;

/// Why an accounts call failed.
#[derive(Debug, thiserror::Error)]
pub enum AccountsError {
    #[error("no account {account} is known")]
    UnknownAccount { account: AccountId },
    #[error("account {account} is signed out")]
    NotSignedIn { account: AccountId },
    #[error("account {account} has no offered manifest key with that fingerprint")]
    NothingToTrust { account: AccountId },
    #[error("account {account} holds no manifest key to verify a manifest with")]
    NoManifestKey { account: AccountId },
    #[error("the instance of account {account} answered without a manifest")]
    NoManifest { account: AccountId },
    #[error("the OS secure store cannot open the secret right now")]
    SecureStorageUnavailable(#[source] SecureError),
    #[error("the secret could not be sealed")]
    Seal(#[source] SecureError),
    #[error("the account store {} could not be read", path.display())]
    StoreRead {
        path: PathBuf,
        #[source]
        source: SecureError,
    },
    // Why: serde_json's own message can quote the offending value, so only its category and
    // position are kept.
    #[error(
        "the account store {} does not decode ({category} error at line {line}, column {column})",
        path.display()
    )]
    StoreCorrupt {
        path: PathBuf,
        category: &'static str,
        line: usize,
        column: usize,
    },
    #[error(
        "the account store {} has version {found}, which this build does not read",
        path.display()
    )]
    StoreVersion { path: PathBuf, found: u32 },
    #[error("the account store could not be encoded")]
    StoreEncode(#[source] serde_json::Error),
    #[error("the account store {} could not be written", path.display())]
    StoreWrite {
        path: PathBuf,
        #[source]
        source: SecureError,
    },
    #[error("the account store {} could not be set aside", path.display())]
    StoreSetAside {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the instance call failed")]
    Client(#[source] ClientError),
    #[error("no session token is available")]
    Credential(#[source] CredentialError),
}

impl From<ClientError> for AccountsError {
    fn from(error: ClientError) -> Self {
        Self::Client(error)
    }
}

impl From<CredentialError> for AccountsError {
    fn from(error: CredentialError) -> Self {
        Self::Credential(error)
    }
}
