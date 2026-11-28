//! Linux: systemd user units, `pkexec` and a policy file under `/etc`.
//!
//! A machine with no user manager (a container, a session started without
//! one) still takes the unit files; the entry is reported inactive and starts
//! once a manager exists. A machine without `pkexec` has no prompt to show, so
//! an elevation request ends with the command to run by hand.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::{env, iter};

use super::process::{self, Finished};
use super::{Current, System, unix};
use crate::elevation::{self, ElevatedCommand, Elevation, ElevationError, ElevationOutcome};
use crate::policy::{PolicyContent, PolicyError, PolicyIdentity, PolicyLocation, PolicyRead};
use crate::scheduler::{
    Created, EntryId, EntryState, Removal, SchedulerEntry, SchedulerError, Trigger, systemd,
};

const PKEXEC: [&str; 2] = ["/usr/bin/pkexec", "/bin/pkexec"];

// Why: pkexec exits 126 when the person dismisses the authentication dialog.
const PKEXEC_DISMISSED: i32 = 126;

const POLICY_ROOT: &str = "/etc";

const SYSTEMCTL: &str = "systemctl";

const CONFIG_HOME: &str = "XDG_CONFIG_HOME";

struct Units {
    service: PathBuf,
    service_name: String,
    timer: PathBuf,
    timer_name: String,
}

// Why: systemd reads user units from $XDG_CONFIG_HOME/systemd/user and falls
// back to ~/.config only when the variable is unset or not absolute.
fn units(id: &EntryId) -> Result<Units, SchedulerError> {
    let config = match env::var_os(CONFIG_HOME).map(PathBuf::from) {
        Some(path) if path.is_absolute() => path,
        _ => unix::user()
            .map_err(|source| SchedulerError::Home { source })?
            .dir
            .join(".config"),
    };
    let directory = config.join("systemd").join("user");
    let service_name = systemd::service_name(id);
    let timer_name = systemd::timer_name(id);
    Ok(Units {
        service: directory.join(&service_name),
        timer: directory.join(&timer_name),
        service_name,
        timer_name,
    })
}

fn systemctl(arguments: &[&str]) -> Result<(), SchedulerError> {
    let command = format!("{SYSTEMCTL} --user {}", arguments.join(" "));
    let finished = process::run(
        OsStr::new(SYSTEMCTL),
        iter::once("--user").chain(arguments.iter().copied()),
    )
    .map_err(|source| SchedulerError::Spawn {
        command: command.clone(),
        source,
    })?;
    if finished.succeeded() {
        return Ok(());
    }
    Err(SchedulerError::Exited {
        command,
        code: finished.code,
        stderr: finished.stderr,
    })
}

// Why: systemctl reports an absent user manager only through its exit text;
// no exit code tells it from a manager that refused the request.
fn no_user_manager(error: &SchedulerError) -> bool {
    match error {
        SchedulerError::Spawn { source, .. } => source.kind() == std::io::ErrorKind::NotFound,
        SchedulerError::Exited { stderr, .. } => {
            stderr.contains("Failed to connect to bus")
                || stderr.contains("No such file or directory")
                || stderr.contains("not been booted with systemd")
        },
        SchedulerError::Home { .. } | SchedulerError::Io { .. } => false,
    }
}

fn unless_no_manager(
    result: Result<(), SchedulerError>,
) -> Result<Option<SchedulerError>, SchedulerError> {
    match result {
        Ok(()) => Ok(None),
        Err(error) if no_user_manager(&error) => Ok(Some(error)),
        Err(error) => Err(error),
    }
}

fn exists(path: &Path) -> Result<bool, SchedulerError> {
    path.try_exists()
        .map_err(|source| SchedulerError::io("inspect", path, source))
}

fn retire(path: &Path, unit: &str) -> Result<(), SchedulerError> {
    if !exists(path)? {
        return Ok(());
    }
    if let Some(cause) = unless_no_manager(systemctl(&["disable", "--now", unit]))? {
        tracing::debug!(unit, error = %cause, "no user manager to disable the unit in");
    }
    unix::remove_if_present(path).map_err(|source| SchedulerError::io("remove", path, source))
}

fn write(path: &Path, contents: &str) -> Result<(), SchedulerError> {
    unix::write_replacing(path, contents.as_bytes())
        .map_err(|source| SchedulerError::io("write", path, source))
}

fn outcome(finished: Finished) -> ElevationOutcome {
    match finished.code {
        Some(0) => ElevationOutcome::Succeeded,
        Some(PKEXEC_DISMISSED) => ElevationOutcome::Declined,
        code => ElevationOutcome::Failed {
            code,
            detail: finished.stderr,
        },
    }
}

fn policy_file(identity: &PolicyIdentity) -> PathBuf {
    Path::new(POLICY_ROOT)
        .join(identity.name())
        .join(identity.file_name())
}

impl System for Current {
    fn elevation() -> Result<Elevation, ElevationError> {
        Ok(unix::elevation())
    }

    fn elevate(command: &ElevatedCommand) -> Result<ElevationOutcome, ElevationError> {
        let Some(pkexec) = PKEXEC.iter().map(Path::new).find(|path| path.is_file()) else {
            return Ok(ElevationOutcome::Manual {
                command: elevation::manual_command(command),
            });
        };
        let arguments =
            iter::once(command.program()).chain(command.arguments().iter().map(String::as_str));
        process::run(pkexec.as_os_str(), arguments)
            .map(outcome)
            .map_err(|source| ElevationError::Spawn {
                command: pkexec.display().to_string(),
                source,
            })
    }

    fn create_entry(entry: &SchedulerEntry) -> Result<Created, SchedulerError> {
        let rendered = systemd::render(entry);
        let units = units(entry.id())?;
        write(&units.service, &rendered.service)?;
        match &rendered.timer {
            Some(timer) => write(&units.timer, timer)?,
            None => retire(&units.timer, &units.timer_name)?,
        }
        if let Some(cause) = unless_no_manager(systemctl(&["daemon-reload"]))? {
            return Ok(Created::Inactive { cause });
        }
        match entry.trigger() {
            Trigger::Every(_) => systemctl(&["enable", "--now", &units.timer_name])?,
            Trigger::AtLogin => systemctl(&["enable", "--now", &units.service_name])?,
            Trigger::OnDemand => {},
        }
        Ok(Created::Active)
    }

    fn query_entry(id: &EntryId) -> Result<EntryState, SchedulerError> {
        Ok(if exists(&units(id)?.service)? {
            EntryState::Installed
        } else {
            EntryState::NotInstalled
        })
    }

    fn remove_entry(id: &EntryId) -> Result<Removal, SchedulerError> {
        let units = units(id)?;
        if !exists(&units.service)? && !exists(&units.timer)? {
            return Ok(Removal::NotInstalled);
        }
        retire(&units.timer, &units.timer_name)?;
        retire(&units.service, &units.service_name)?;
        if let Some(cause) = unless_no_manager(systemctl(&["daemon-reload"]))? {
            tracing::debug!(error = %cause, "no user manager to reload after the removal");
        }
        Ok(Removal::Removed)
    }

    fn policy_locations(identity: &PolicyIdentity) -> Result<Vec<PolicyLocation>, PolicyError> {
        Ok(vec![PolicyLocation::File {
            path: policy_file(identity),
        }])
    }

    fn read_policy(identity: &PolicyIdentity) -> Result<Vec<PolicyRead>, PolicyError> {
        let path = policy_file(identity);
        Ok(unix::read_policy_file(&path, unix::root_owned)?
            .map(|bytes| PolicyRead {
                location: PolicyLocation::File { path },
                content: PolicyContent::Document(bytes),
            })
            .into_iter()
            .collect())
    }
}
