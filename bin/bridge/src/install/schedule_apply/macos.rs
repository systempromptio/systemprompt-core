//! launchd registration for the sync job and the proxy supervisor.
//!
//! Both agents live in the per-user `gui/<uid>` domain. The proxy agent is what
//! keeps `127.0.0.1:<port>` answering when the desktop GUI is not running —
//! without it, every client config the bridge writes (host profiles, plugin
//! `hooks.json`, `.mcp.json`) names a port nothing is listening on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{InstallError, ScheduleRemoval, home, proxy_registration, remove_if_present, write};
use crate::schedule::{self, Os};

// Why: launchd addresses the per-user domain as `gui/<uid>`.
pub(super) fn gui_domain() -> String {
    #![allow(unsafe_code, reason = "libc::getuid is the only way to read the uid")]
    // SAFETY: `getuid` takes no arguments, cannot fail, and always returns the
    // calling process's real uid.
    format!("gui/{}", unsafe { libc::getuid() })
}

pub(super) fn agents_dir() -> Result<PathBuf, InstallError> {
    Ok(home()?.join("Library").join("LaunchAgents"))
}

pub(super) fn register(
    os: Os,
    rendered: &str,
    binary: &Path,
) -> Result<(PathBuf, Vec<String>), InstallError> {
    let dir = agents_dir()?;
    let label = schedule::schedule_label(os);
    let path = dir.join(format!("{label}.plist"));
    write(&path, rendered)?;
    bootstrap(&path)?;

    let domain = gui_domain();
    let mut lines = vec![
        format!("wrote: {}", path.display()),
        format!("launchd agent: {label} (loaded in {domain})"),
    ];

    if let Some((proxy_path, body)) = proxy_registration(os, binary, &dir) {
        write(&proxy_path, &body)?;
        bootstrap(&proxy_path)?;
        let proxy_label = schedule::proxy_job_name(os);
        lines.push(format!("wrote: {}", proxy_path.display()));
        lines.push(format!(
            "launchd agent: {proxy_label} (loaded in {domain}, restarts on exit)"
        ));
    }

    Ok((path, lines))
}

fn bootstrap(path: &Path) -> Result<(), InstallError> {
    let domain = gui_domain();
    // Why: launchctl fails bootout with "not loaded" on a first install, which
    // is expected and not fatal.
    _ = Command::new("launchctl")
        .args(["bootout", &domain])
        .arg(path)
        .status();
    let status = Command::new("launchctl")
        .args(["bootstrap", &domain])
        .arg(path)
        .status()
        .map_err(|e| InstallError::ScheduleApply(format!("launchctl bootstrap: {e}")))?;
    if status.success() {
        return Ok(());
    }
    Err(InstallError::ScheduleApply(format!(
        "launchctl bootstrap {} exited with {}",
        path.display(),
        status.code().unwrap_or(-1)
    )))
}

fn kickstart(label: &str) -> bool {
    Command::new("launchctl")
        .args(["kickstart", "-k", &format!("{}/{label}", gui_domain())])
        .status()
        .is_ok_and(|s| s.success())
}

pub(super) fn ensure_proxy(os: Os, binary: &Path) -> Result<bool, InstallError> {
    let dir = agents_dir()?;
    let Some((path, body)) = proxy_registration(os, binary, &dir) else {
        return Ok(false);
    };
    if std::fs::read_to_string(&path).ok().as_deref() != Some(body.as_str()) {
        write(&path, &body)?;
        bootstrap(&path)?;
        return Ok(true);
    }
    if kickstart(&schedule::proxy_job_name(os)) {
        return Ok(true);
    }
    // Why: kickstart fails when the plist is on disk but not loaded in the
    // domain — the state a reboot before `bootstrap` leaves behind.
    bootstrap(&path)?;
    Ok(true)
}

pub(super) fn remove_current() -> ScheduleRemoval {
    let label = schedule::schedule_label(Os::Mac);
    let Ok(dir) = agents_dir() else {
        return ScheduleRemoval::Failed("cannot resolve the user's home directory".into());
    };
    let path = dir.join(format!("{label}.plist"));
    let proxy_label = schedule::proxy_job_name(Os::Mac);
    let proxy_path = dir.join(schedule::proxy_template_filename(Os::Mac));
    if !path.exists() && !proxy_path.exists() {
        return ScheduleRemoval::NotInstalled(label.to_owned());
    }

    let domain = gui_domain();
    for agent in [&path, &proxy_path] {
        _ = Command::new("launchctl")
            .args(["bootout", &domain])
            .arg(agent)
            .status();
    }
    match remove_if_present(&path).and_then(|()| remove_if_present(&proxy_path)) {
        Ok(()) => ScheduleRemoval::Removed(format!("{label} + {proxy_label}")),
        Err(e) => ScheduleRemoval::Failed(format!("remove under {}: {e}", dir.display())),
    }
}
