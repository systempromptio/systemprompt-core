//! The profile index: the top-level config file, holding only `active`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use toml_edit::DocumentMut;

use super::{GatewayProfileId, ProfileError};

pub(super) const ACTIVE_KEY: &str = "active";

pub(super) fn read_doc(path: &Path) -> Result<Option<DocumentMut>, ProfileError> {
    let Some(body) =
        crate::fsutil::read_optional(path).map_err(|e| ProfileError::io("read", path, e))?
    else {
        return Ok(None);
    };
    body.parse::<DocumentMut>()
        .map(Some)
        .map_err(|source| ProfileError::Malformed {
            path: path.to_path_buf(),
            source: Box::new(source),
        })
}

pub(super) fn active_of(
    path: &Path,
    doc: &DocumentMut,
) -> Result<Option<GatewayProfileId>, ProfileError> {
    let Some(value) = doc.get(ACTIVE_KEY).and_then(|i| i.as_str()) else {
        return Ok(None);
    };
    GatewayProfileId::try_new(value)
        .map(Some)
        .map_err(|source| ProfileError::BadActive {
            path: path.to_path_buf(),
            source,
        })
}

pub(super) fn read_active(path: &Path) -> Result<Option<GatewayProfileId>, ProfileError> {
    read_doc(path)?.map_or(Ok(None), |doc| active_of(path, &doc))
}

pub(super) fn write_active(path: &Path, id: &GatewayProfileId) -> Result<(), ProfileError> {
    let mut doc = DocumentMut::new();
    doc.insert(ACTIVE_KEY, toml_edit::value(id.as_str()));
    if let Some(parent) = path.parent() {
        crate::fsutil::create_dir_all_mode_0700(parent)
            .map_err(|e| ProfileError::io("create", parent, e))?;
    }
    crate::fsutil::atomic_write_0600(path, doc.to_string().as_bytes())
        .map_err(|e| ProfileError::io("write", path, e))
}
