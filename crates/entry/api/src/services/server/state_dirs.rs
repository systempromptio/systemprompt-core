//! Runtime-state directories must be writable by the server process, checked
//! once before any route or extension initialises.
//!
//! Since 0.58.1 the gateway journal lives under `{paths.storage}/data`. An
//! image that does not create that directory, with a volume mounted there,
//! gives a root-owned mount to a server running as an unprivileged user; the
//! boot then died inside the gateway extension with `Cannot create gateway
//! journal at …` and no cause. This check creates and write-probes each
//! directory, reports every one that fails in one error, and names the
//! directory's owner, the process's uid and the fix.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Result, anyhow};

const PROBE: &str = ".systemprompt-write-probe";

static PROBE_SEQ: AtomicU64 = AtomicU64::new(0);

pub(crate) fn create_state_dir(dir: &Path) -> Result<()> {
    let probe = dir.join(format!(
        "{PROBE}-{}-{}",
        std::process::id(),
        PROBE_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let outcome = std::fs::create_dir_all(dir)
        .and_then(|()| std::fs::write(&probe, b""))
        .and_then(|()| std::fs::remove_file(&probe));
    outcome.map_err(|error| anyhow!("{}: {error}{}", dir.display(), ownership(dir)))
}

pub(crate) fn ensure_writable(dirs: &[&Path]) -> Result<()> {
    let failures: Vec<String> = dirs
        .iter()
        .filter_map(|dir| create_state_dir(dir).err().map(|e| format!("  - {e}")))
        .collect();
    if failures.is_empty() {
        return Ok(());
    }
    Err(anyhow!(
        "Runtime-state directories are not writable by this process:\n{}\nMount each as a \
         persistent volume writable by the server's uid (in the published image: uid 1000; on \
         Kubernetes set securityContext.fsGroup: 1000).",
        failures.join("\n")
    ))
}

#[cfg(unix)]
fn ownership(dir: &Path) -> String {
    use std::os::unix::fs::MetadataExt;
    let uid = nix::unistd::geteuid();
    // Why: when `dir` itself cannot be created, the nearest existing ancestor
    // is what refused it.
    let owner = dir
        .ancestors()
        .find_map(|p| std::fs::metadata(p).ok().map(|m| (p.to_path_buf(), m)))
        .map(|(p, m)| {
            format!(
                "; {} is owned by uid {} gid {} mode {:o}",
                p.display(),
                m.uid(),
                m.gid(),
                m.mode() & 0o7777
            )
        })
        .unwrap_or_default();
    format!(" (running as uid {uid}{owner}; the mount must be writable by uid {uid})")
}

#[cfg(not(unix))]
fn ownership(_dir: &Path) -> String {
    String::new()
}
