//! systemd user-unit registration for the sync timer and the proxy supervisor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use super::{InstallError, ScheduleRemoval, home, write};
use crate::schedule::{self, Os};

pub(super) fn register(
    os: Os,
    rendered: &str,
    binary: &Path,
) -> Result<(PathBuf, Vec<String>), InstallError> {
    let unit = schedule::schedule_label(os);
    let (service, timer) = schedule::split_systemd_unit(rendered).ok_or_else(|| {
        InstallError::ScheduleApply("systemd template has no .timer section".into())
    })?;
    let dir = home()?.join(".config").join("systemd").join("user");
    let service_path = dir.join(format!("{unit}.service"));
    let timer_path = dir.join(format!("{unit}.timer"));
    write(&service_path, &service)?;
    write(&timer_path, &timer)?;

    let proxy_unit = schedule::proxy_unit_name();
    let proxy_path = dir.join(format!("{proxy_unit}.service"));
    write(&proxy_path, &schedule::proxy_template(binary))?;

    let mut lines = vec![
        format!("wrote: {}", service_path.display()),
        format!("wrote: {}", timer_path.display()),
        format!("wrote: {}", proxy_path.display()),
    ];

    // Why: containers and WSL distributions without a user manager can
    // still hold the unit files; the operator activates them once systemd
    // --user exists. That is a degraded install, not a failed one — the
    // receipts stand and the caller's `set -e` must not abort on it — so
    // only an activation that a live user manager refused is an error.
    match activate(unit, &proxy_unit) {
        Ok(()) => {
            lines.push(format!(
                "systemd user timer: {unit}.timer (enabled, every 30m)"
            ));
            lines.push(format!(
                "systemd user service: {proxy_unit}.service (enabled, restarts on failure)"
            ));
        },
        Err(Activation::NoUserManager(reason)) => {
            lines.push(format!(
                "systemd --user is not available here ({reason}); units written but not \
                 activated — run `systemctl --user daemon-reload && systemctl --user enable \
                 --now {unit}.timer {proxy_unit}.service` where it is, or start `astound-bridge \
                 proxy` by hand"
            ));
        },
        Err(Activation::Refused(e)) => {
            return Err(InstallError::ScheduleActivation {
                units: vec![service_path, timer_path, proxy_path],
                reason: e.to_string(),
            });
        },
    }
    Ok((timer_path, lines))
}

enum Activation {
    NoUserManager(String),
    Refused(InstallError),
}

// Why: only a genuinely absent user manager degrades — `systemctl` not on
// PATH, or a reload that could not reach the user bus at all. A live manager
// that refuses the reload (polkit, a unit it rejects) is an activation
// failure, or the timer would be silently missing on a host that has one.
fn activate(unit: &str, proxy_unit: &str) -> Result<(), Activation> {
    systemctl(&["daemon-reload"]).map_err(|e| {
        if e.no_user_manager() {
            Activation::NoUserManager(e.to_string())
        } else {
            Activation::Refused(e.into_install_error())
        }
    })?;
    systemctl(&["enable", "--now", &format!("{unit}.timer")])
        .map_err(|e| Activation::Refused(e.into_install_error()))?;
    systemctl(&["enable", "--now", &format!("{proxy_unit}.service")])
        .map_err(|e| Activation::Refused(e.into_install_error()))
}

enum SystemctlFailure {
    Spawn {
        command: String,
        source: std::io::Error,
    },
    Exited {
        command: String,
        code: i32,
        stderr: String,
    },
}

impl SystemctlFailure {
    fn no_user_manager(&self) -> bool {
        match self {
            Self::Spawn { source, .. } => source.kind() == std::io::ErrorKind::NotFound,
            Self::Exited { stderr, .. } => {
                stderr.contains("Failed to connect to bus")
                    || stderr.contains("No such file or directory")
                    || stderr.contains("not been booted with systemd")
            },
        }
    }

    fn into_install_error(self) -> InstallError {
        InstallError::ScheduleApply(self.to_string())
    }
}

impl std::fmt::Display for SystemctlFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn { command, source } => write!(f, "{command}: {source}"),
            Self::Exited {
                command,
                code,
                stderr,
            } => {
                let stderr = stderr.trim();
                if stderr.is_empty() {
                    write!(f, "{command} exited with {code}")
                } else {
                    write!(f, "{command} exited with {code}: {stderr}")
                }
            },
        }
    }
}

fn systemctl(args: &[&str]) -> Result<(), SystemctlFailure> {
    let command = format!("systemctl --user {}", args.join(" "));
    let output = std::process::Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|source| SystemctlFailure::Spawn {
            command: command.clone(),
            source,
        })?;
    if output.status.success() {
        return Ok(());
    }
    Err(SystemctlFailure::Exited {
        command,
        code: output.status.code().unwrap_or(-1),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub(super) fn schedule_registered() -> super::ScheduleStatus {
    let unit = schedule::schedule_label(Os::Linux);
    home().map_or(super::ScheduleStatus::Unknown, |h| {
        let timer = h
            .join(".config")
            .join("systemd")
            .join("user")
            .join(format!("{unit}.timer"));
        if timer.exists() {
            super::ScheduleStatus::Installed
        } else {
            super::ScheduleStatus::NotInstalled
        }
    })
}

pub(super) fn remove_current() -> ScheduleRemoval {
    let unit = schedule::schedule_label(Os::Linux);
    let proxy_unit = schedule::proxy_unit_name();
    let Ok(home) = home() else {
        return ScheduleRemoval::Failed("cannot resolve the user's home directory".into());
    };
    let dir = home.join(".config").join("systemd").join("user");
    let timer_path = dir.join(format!("{unit}.timer"));
    let proxy_path = dir.join(format!("{proxy_unit}.service"));
    if !timer_path.exists() && !proxy_path.exists() {
        return ScheduleRemoval::NotInstalled(unit.to_owned());
    }
    if let Err(e) = stop_if_present(&timer_path, &format!("{unit}.timer"))
        .and_then(|()| stop_if_present(&proxy_path, &format!("{proxy_unit}.service")))
    {
        return ScheduleRemoval::Failed(e);
    }
    let removed = remove_if_present(&timer_path)
        .and_then(|()| remove_if_present(&dir.join(format!("{unit}.service"))))
        .and_then(|()| remove_if_present(&proxy_path));
    if let Err(e) = removed {
        return ScheduleRemoval::Failed(format!("remove under {}: {e}", dir.display()));
    }
    if let Err(e) = systemctl(&["daemon-reload"]) {
        return ScheduleRemoval::Failed(format!("reload systemd: {e}"));
    }
    ScheduleRemoval::Removed(format!("{unit} + {proxy_unit}"))
}

fn stop_if_present(path: &Path, unit: &str) -> Result<(), String> {
    let present = path
        .try_exists()
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    if present {
        systemctl(&["disable", "--now", unit]).map_err(|e| format!("stop {unit}: {e}"))?;
    }
    Ok(())
}

fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

pub(super) const fn register_autostart(_rendered: &str) -> Result<Vec<String>, InstallError> {
    Err(InstallError::ScheduleOsMismatch)
}

pub(super) fn remove_autostart() -> ScheduleRemoval {
    ScheduleRemoval::NotInstalled(schedule::autostart_label(Os::Linux).to_owned())
}

pub(super) fn autostart_status() -> super::ScheduleStatus {
    tracing::debug!("autostart unavailable: this platform has no desktop shell");
    super::ScheduleStatus::NotInstalled
}
