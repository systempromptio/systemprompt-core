//! systemd user-unit registration for the sync timer and the proxy supervisor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use super::{
    InstallError, ScheduleRemoval, home, proxy_registration, remove_if_present, write,
};
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

    let proxy_unit = schedule::proxy_job_name(os);
    let mut lines = vec![
        format!("wrote: {}", service_path.display()),
        format!("wrote: {}", timer_path.display()),
    ];
    if let Some((proxy_path, body)) = proxy_registration(os, binary, &dir) {
        write(&proxy_path, &body)?;
        lines.push(format!("wrote: {}", proxy_path.display()));
    }

    // Why: activation needs a systemd user bus, which containers and
    // systemd-less WSL distros lack; the written units still stand.
    if let Err(e) = activate(unit, &proxy_unit) {
        crate::obs::output::diag(&format!(
            "warning: units written but not activated: {e}. Activate them yourself with: \
             systemctl --user daemon-reload && systemctl --user enable --now {unit}.timer \
             {proxy_unit}.service"
        ));
        lines.push(format!("not activated: {e}"));
        return Ok((timer_path, lines));
    }

    lines.push(format!(
        "systemd user timer: {unit}.timer (enabled, every 30m)"
    ));
    lines.push(format!(
        "systemd user service: {proxy_unit}.service (enabled, restarts on failure)"
    ));
    Ok((timer_path, lines))
}

fn activate(unit: &str, proxy_unit: &str) -> Result<(), InstallError> {
    systemctl(&["daemon-reload"])?;
    systemctl(&["enable", "--now", &format!("{unit}.timer")])?;
    systemctl(&["enable", "--now", &format!("{proxy_unit}.service")])
}

pub(super) fn ensure_proxy(os: Os, binary: &Path) -> Result<bool, InstallError> {
    let dir = home()?.join(".config").join("systemd").join("user");
    let Some((path, body)) = proxy_registration(os, binary, &dir) else {
        return Ok(false);
    };
    let unit = format!("{}.service", schedule::proxy_job_name(os));
    if std::fs::read_to_string(&path).ok().as_deref() != Some(body.as_str()) {
        write(&path, &body)?;
        systemctl(&["daemon-reload"])?;
    }
    systemctl(&["enable", "--now", &unit])?;
    Ok(true)
}

fn systemctl(args: &[&str]) -> Result<(), InstallError> {
    let status = std::process::Command::new("systemctl")
        .arg("--user")
        .args(args)
        .status()
        .map_err(|e| InstallError::ScheduleApply(format!("systemctl --user {}: {e}", args[0])))?;
    if status.success() {
        return Ok(());
    }
    Err(InstallError::ScheduleApply(format!(
        "systemctl --user {} exited with {}",
        args.join(" "),
        status.code().unwrap_or(-1)
    )))
}

pub(super) fn remove_current() -> ScheduleRemoval {
    let unit = schedule::schedule_label(Os::Linux);
    let proxy_unit = schedule::proxy_job_name(Os::Linux);
    let Ok(home) = home() else {
        return ScheduleRemoval::Failed("cannot resolve the user's home directory".into());
    };
    let dir = home.join(".config").join("systemd").join("user");
    let timer_path = dir.join(format!("{unit}.timer"));
    let proxy_path = dir.join(format!("{proxy_unit}.service"));
    if !timer_path.exists() && !proxy_path.exists() {
        return ScheduleRemoval::NotInstalled(unit.to_owned());
    }
    _ = systemctl(&["disable", "--now", &format!("{unit}.timer")]);
    _ = systemctl(&["disable", "--now", &format!("{proxy_unit}.service")]);
    let removed = remove_if_present(&timer_path)
        .and_then(|()| remove_if_present(&dir.join(format!("{unit}.service"))))
        .and_then(|()| remove_if_present(&proxy_path));
    match removed {
        Ok(()) => {
            _ = systemctl(&["daemon-reload"]);
            ScheduleRemoval::Removed(format!("{unit} + {proxy_unit}"))
        },
        Err(e) => ScheduleRemoval::Failed(format!("remove under {}: {e}", dir.display())),
    }
}
