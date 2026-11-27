//! What [`crate::plan`] reads: the verified manifest and what was fetched with
//! it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::fmt;

use sp_protocol::ids::{InstanceKey, PluginId};
use sp_protocol::wire::catalog::{ModelCatalog, Profile};
use sp_protocol::wire::manifest::Manifest;

/// Plugin bundle file bytes as delivered, keyed by plugin id and the file's
/// path inside the bundle.
///
/// The projection checks every file against the digest the signed manifest
/// gives it; nothing here is trusted as delivered.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct BundleFiles {
    files: BTreeMap<(String, String), Vec<u8>>,
}

impl BundleFiles {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, plugin: &PluginId, path: impl Into<String>, bytes: Vec<u8>) {
        self.files
            .insert((plugin.as_str().to_owned(), path.into()), bytes);
    }

    #[must_use]
    pub fn get(&self, plugin: &PluginId, path: &str) -> Option<&[u8]> {
        self.files
            .get(&(plugin.as_str().to_owned(), path.to_owned()))
            .map(Vec::as_slice)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, &str, &[u8])> {
        self.files
            .iter()
            .map(|((plugin, path), bytes)| (plugin.as_str(), path.as_str(), bytes.as_slice()))
    }
}

impl fmt::Debug for BundleFiles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BundleFiles")
            .field("files", &self.files.len())
            .finish()
    }
}

/// Everything a projection is computed from.
///
/// `manifest` must already be verified and accepted;
/// `host` is the host id the harness presents (`sp_protocol::HOST_ID`);
/// `instance_key` is the slug of the account label.
#[derive(Debug, Clone, Copy)]
pub struct ProjectionInput<'a> {
    pub manifest: &'a Manifest,
    pub profile: Option<&'a Profile>,
    pub catalog: Option<&'a ModelCatalog>,
    pub instance_url: &'a str,
    pub instance_key: &'a InstanceKey,
    pub host: &'a str,
    pub bundle_files: &'a BundleFiles,
}
