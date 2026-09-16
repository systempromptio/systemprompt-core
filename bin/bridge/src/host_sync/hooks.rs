//! Host attribution for emitted `hooks.json` files: the org-plugins source is
//! written unstamped and each host emitter stamps the copy it hands over.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use super::ApplyError;
use super::hooks_schema::HooksFile;
use crate::fsutil::atomic_write_0644;

// Why: a missing file is not an error — plugins without hooks have nothing to
// stamp.
pub fn stamp_hooks_file(path: &Path, host: &str) -> Result<(), ApplyError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(ApplyError::Io {
                context: format!("read {}", path.display()),
                source: e,
            });
        },
    };
    let mut file: HooksFile = serde_json::from_slice(&bytes).map_err(|e| ApplyError::Io {
        context: format!("parse {}", path.display()),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
    })?;
    file.stamp_host(host);
    let stamped = serde_json::to_vec_pretty(&file).map_err(|e| ApplyError::Serialize {
        what: format!("stamped {}", path.display()),
        source: e,
    })?;
    atomic_write_0644(path, &stamped).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}
