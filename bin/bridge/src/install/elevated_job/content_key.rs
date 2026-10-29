//! The key an elevated job is single-flighted under: what it writes, not
//! where its staged copies happen to live.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use super::ElevatedJob;

// Why: staged files live in per-call temporary directories, so two requests
// for the same change differ in their paths; the key names what is written.
pub(super) fn content_key(job: &ElevatedJob) -> std::io::Result<String> {
    let digest = |path: &Path| std::fs::read(path).map(|bytes| crate::hash::sha256_hex(&bytes));
    let reg = job
        .reg_path
        .as_deref()
        .map(|p| digest(Path::new(p)))
        .transpose()?;
    let files = job
        .managed_files
        .iter()
        .map(|file| Ok((file.dest.clone(), digest(&file.staged)?)))
        .collect::<std::io::Result<Vec<_>>>()?;
    let dirs: Vec<_> = job
        .private_dirs
        .iter()
        .map(|dir| (&dir.path, &dir.owner_sid))
        .collect();
    let described = serde_json::to_vec(&(
        reg,
        job.org_plugins.as_ref().map(|o| (&o.path, &o.grant_user)),
        &job.clear_values,
        &job.bridge_values,
        files,
        &job.remove_files,
        dirs,
    ))
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(crate::hash::sha256_hex(&described))
}
