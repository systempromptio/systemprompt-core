//! Which MCP connector entries the bridge wrote into a host's own config
//! file, recorded beside that file so the next sync removes exactly those.
//!
//! Ownership is recorded, never inferred from the entry's URL: after a proxy
//! port move the old entries still point at the previous origin and would
//! otherwise linger as foreign. A sidecar that cannot be read is an error —
//! a corrupt record must not read as "nothing is ours".
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::host_sync::ApplyError;

pub const SIDECAR_FILE: &str = ".systemprompt-mcp.json";

#[derive(Debug, Default, Serialize, Deserialize)]
struct Recorded {
    #[serde(default)]
    slugs: Vec<String>,
}

#[must_use]
pub fn beside(config_file: &Path) -> PathBuf {
    config_file.with_file_name(SIDECAR_FILE)
}

pub fn read(sidecar: &Path) -> Result<Vec<String>, ApplyError> {
    let Some(text) = crate::fsutil::read_optional(sidecar).map_err(|source| ApplyError::Io {
        context: format!("read {}", sidecar.display()),
        source,
    })?
    else {
        return Ok(Vec::new());
    };
    serde_json::from_str::<Recorded>(&text)
        .map(|r| r.slugs)
        .map_err(|source| ApplyError::Serialize {
            what: format!(
                "{} is corrupt; refusing to treat a corrupt sidecar as empty",
                sidecar.display()
            ),
            source,
        })
}

pub fn write(sidecar: &Path, slugs: &[String]) -> Result<(), ApplyError> {
    if slugs.is_empty() {
        return crate::fsutil::remove_verified(sidecar).map_err(|source| ApplyError::Io {
            context: format!("remove {}", sidecar.display()),
            source,
        });
    }
    let bytes = serde_json::to_vec_pretty(&Recorded {
        slugs: slugs.to_vec(),
    })
    .map_err(|source| ApplyError::Serialize {
        what: "mcp sidecar".to_owned(),
        source,
    })?;
    crate::fsutil::atomic_write_0600(sidecar, &bytes).map_err(|source| ApplyError::Io {
        context: format!("write {}", sidecar.display()),
        source,
    })
}
