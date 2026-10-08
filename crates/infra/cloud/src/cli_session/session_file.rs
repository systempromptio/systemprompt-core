//! On-disk persistence of one [`CliSession`] record.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use super::session::{CURRENT_VERSION, CliSession, MIN_SUPPORTED_VERSION};
use crate::error::{CloudError, CloudResult};
use crate::private_dir::write_private_json;

impl CliSession {
    pub fn load_from_path(path: &Path) -> CloudResult<Self> {
        if !path.exists() {
            return Err(CloudError::NotAuthenticated);
        }

        let content = fs::read_to_string(path)?;

        let mut session: Self =
            serde_json::from_str(&content).map_err(|e| CloudError::CredentialsCorrupted {
                source: Box::new(e),
            })?;

        if session.version < MIN_SUPPORTED_VERSION || session.version > CURRENT_VERSION {
            return Err(CloudError::SessionVersionMismatch {
                min: MIN_SUPPORTED_VERSION,
                max: CURRENT_VERSION,
                actual: session.version,
                path: path.display().to_string(),
            });
        }

        session.version = CURRENT_VERSION;
        Ok(session)
    }

    pub fn save_to_path(&self, path: &Path) -> CloudResult<()> {
        write_private_json(path, self)
    }

    pub fn delete_from_path(path: &Path) -> CloudResult<()> {
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}
