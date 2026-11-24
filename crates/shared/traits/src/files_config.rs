//! The files configuration facts a host reads without naming the files member.
//!
//! They are the public `/files` URL prefix, the writable files root,
//! the configured `Cache-Control` of stored files and whether the document
//! store is enabled. The files member provides it open
//! (`Arc<FilesConfigView>`), built from its loaded configuration; with the
//! member absent the host serves no stored files and mounts no document
//! routes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

/// The files configuration as the API host reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesConfigView {
    pub url_prefix: String,
    pub files_root: PathBuf,
    pub cache_control: Option<String>,
    pub documents_enabled: bool,
}
