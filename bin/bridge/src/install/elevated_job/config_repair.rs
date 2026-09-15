//! Reassigns the bridge configuration directory to the requesting account
//! through an elevated job when the current process cannot write it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use super::{ElevatedJob, PrivateDirJob, elevate_and_run};

pub(crate) fn repair_config_dir_elevated() -> std::io::Result<PathBuf> {
    let config = crate::config::config_path()
        .ok_or_else(|| std::io::Error::other("config path unresolvable on this platform"))?;
    let dir = config
        .parent()
        .map(Path::to_owned)
        .ok_or_else(|| std::io::Error::other("config path has no parent directory"))?;
    let owner_sid = crate::windows_acl::current_sid()?;
    let stage_dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
    std::fs::create_dir_all(&stage_dir)?;
    let job = ElevatedJob {
        private_dirs: vec![PrivateDirJob {
            path: dir.clone(),
            owner_sid,
        }],
        ..Default::default()
    };
    elevate_and_run(&stage_dir, &job)?.require("own", &dir)?;
    Ok(dir)
}
