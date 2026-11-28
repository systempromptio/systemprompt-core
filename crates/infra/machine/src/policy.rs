//! Where a managed configuration is read from on each system, and its
//! content as found.
//!
//! Windows reads the values of `HKLM\SOFTWARE\Policies\<name>` and a file under
//! the machine's `ProgramData`; macOS reads the managed preferences profile of
//! the domain, for the user and for the machine; Linux reads a file under
//! `/etc`. Nothing here writes. A file is returned as the bytes found; the
//! document format belongs to the store that reads it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::platform::{Current, System};

pub(crate) const DOCUMENT_LIMIT: u64 = 4 * 1024 * 1024;

const SEGMENT_LIMIT: usize = 128;

/// The names a product's managed configuration is filed under.
///
/// The vendor key (registry key, `ProgramData` and `/etc` directory), the
/// preferences domain and the file name. Each is a single path segment of
/// letters, digits, `.`, `-` and `_`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyIdentity {
    name: String,
    domain: String,
    file_name: String,
}

fn segment(field: &'static str, value: &str) -> Result<String, PolicyError> {
    let plain = value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'));
    if value.is_empty() || value.len() > SEGMENT_LIMIT || value.starts_with('.') || !plain {
        return Err(PolicyError::Identity {
            field,
            value: value.to_owned(),
        });
    }
    Ok(value.to_owned())
}

impl PolicyIdentity {
    pub fn new(name: &str, domain: &str, file_name: &str) -> Result<Self, PolicyError> {
        Ok(Self {
            name: segment("name", name)?,
            domain: segment("domain", domain)?,
            file_name: segment("file name", file_name)?,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }

    #[must_use]
    pub fn file_name(&self) -> &str {
        &self.file_name
    }
}

/// Whose configuration a managed preferences profile governs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyScope {
    Machine,
    User,
}

/// One place a managed configuration is read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyLocation {
    Registry { key: String },
    File { path: PathBuf },
    ManagedPreferences { path: PathBuf, scope: PolicyScope },
}

/// One registry value of a policy key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyValue {
    Text(String),
    Number(u32),
}

/// What a location held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyContent {
    Values(BTreeMap<String, PolicyValue>),
    Document(Vec<u8>),
}

/// A location that exists on this machine, with its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRead {
    pub location: PolicyLocation,
    pub content: PolicyContent,
}

/// Why a managed configuration could not be located or read.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PolicyError {
    #[error("policy {field} {value:?} is not a single plain path segment")]
    Identity { field: &'static str, value: String },
    #[error("{op} {}: {source}", path.display())]
    Io {
        op: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{} is larger than {limit} bytes", path.display())]
    TooLarge { path: PathBuf, limit: u64 },
    #[error("{} is not a regular file only an administrator can write", path.display())]
    Untrusted { path: PathBuf },
    #[error("registry key {key}: {source}")]
    Registry {
        key: String,
        #[source]
        source: io::Error,
    },
    #[error("registry value {name:?} of {key} has unsupported type {kind}")]
    UnsupportedValue {
        key: String,
        name: String,
        kind: u32,
    },
    #[error("resolve the current user: {source}")]
    User {
        #[source]
        source: io::Error,
    },
}

impl PolicyError {
    pub(crate) fn io(op: &'static str, path: &Path, source: io::Error) -> Self {
        Self::Io {
            op,
            path: path.to_path_buf(),
            source,
        }
    }
}

pub fn locations(identity: &PolicyIdentity) -> Result<Vec<PolicyLocation>, PolicyError> {
    Current::policy_locations(identity)
}

pub fn read(identity: &PolicyIdentity) -> Result<Vec<PolicyRead>, PolicyError> {
    Current::read_policy(identity)
}
