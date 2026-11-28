//! macOS: launchd agents in the user's GUI domain, the authorization dialog
//! and managed preferences profiles.
//!
//! The dialog is the one `osascript` raises for a script run with
//! administrator privileges, which works with and without a terminal. A
//! profile is read from `/Library/Managed Preferences`: the one filed under the
//! user's short name governs that user and comes first, the one at the root
//! governs the machine.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use nix::unistd::Uid;

use super::process::{self, Finished};
use super::{Current, System, unix};
use crate::elevation::{self, ElevatedCommand, Elevation, ElevationError, ElevationOutcome};
use crate::policy::{
    PolicyContent, PolicyError, PolicyIdentity, PolicyLocation, PolicyRead, PolicyScope,
};
use crate::scheduler::{
    Created, EntryId, EntryState, Removal, SchedulerEntry, SchedulerError, launchd,
};

const LAUNCHCTL: &str = "/bin/launchctl";

const OSASCRIPT: &str = "/usr/bin/osascript";

const MANAGED_PREFERENCES: &str = "/Library/Managed Preferences";

// Why: AppleScript reports a dismissed authorization dialog as error -128.
const CANCELLED: &str = "(-128)";

// Why: launchctl exits 3 (no such process) when the service it is asked to
// boot out is not loaded.
const NOT_LOADED: i32 = 3;

fn agent(id: &EntryId) -> Result<PathBuf, SchedulerError> {
    Ok(unix::user()
        .map_err(|source| SchedulerError::Home { source })?
        .dir
        .join("Library")
        .join("LaunchAgents")
        .join(launchd::file_name(id)))
}

fn domain() -> String {
    format!("gui/{}", Uid::current())
}

fn launchctl(
    verb: &str,
    target: &OsStr,
    operand: Option<&OsStr>,
) -> Result<Finished, SchedulerError> {
    process::run(
        OsStr::new(LAUNCHCTL),
        [OsStr::new(verb), target].into_iter().chain(operand),
    )
    .map_err(|source| SchedulerError::Spawn {
        command: format!("launchctl {verb}"),
        source,
    })
}

fn exited(verb: &str, finished: Finished) -> SchedulerError {
    SchedulerError::Exited {
        command: format!("launchctl {verb}"),
        code: finished.code,
        stderr: finished.stderr,
    }
}

fn bootout(id: &EntryId) -> Result<(), SchedulerError> {
    let target = format!("{}/{id}", domain());
    let finished = launchctl("bootout", OsStr::new(&target), None)?;
    if finished.succeeded() || finished.code == Some(NOT_LOADED) {
        return Ok(());
    }
    Err(exited("bootout", finished))
}

fn exists(path: &Path) -> Result<bool, SchedulerError> {
    path.try_exists()
        .map_err(|source| SchedulerError::io("inspect", path, source))
}

fn profiles(identity: &PolicyIdentity) -> Result<[(PathBuf, PolicyScope); 2], PolicyError> {
    let user = unix::user().map_err(|source| PolicyError::User { source })?;
    let file = format!("{}.plist", identity.domain());
    let root = Path::new(MANAGED_PREFERENCES);
    Ok([
        (root.join(&user.name).join(&file), PolicyScope::User),
        (root.join(&file), PolicyScope::Machine),
    ])
}

impl System for Current {
    fn elevation() -> Result<Elevation, ElevationError> {
        Ok(unix::elevation())
    }

    fn elevate(command: &ElevatedCommand) -> Result<ElevationOutcome, ElevationError> {
        let script = elevation::authorization_script(command);
        let finished =
            process::run(OsStr::new(OSASCRIPT), ["-e", script.as_str()]).map_err(|source| {
                ElevationError::Spawn {
                    command: OSASCRIPT.to_owned(),
                    source,
                }
            })?;
        Ok(if finished.succeeded() {
            ElevationOutcome::Succeeded
        } else if finished.stderr.contains(CANCELLED) {
            ElevationOutcome::Declined
        } else {
            ElevationOutcome::Failed {
                code: finished.code,
                detail: finished.stderr,
            }
        })
    }

    fn create_entry(entry: &SchedulerEntry) -> Result<Created, SchedulerError> {
        let path = agent(entry.id())?;
        unix::write_replacing(&path, launchd::render(entry).as_bytes())
            .map_err(|source| SchedulerError::io("write", &path, source))?;
        bootout(entry.id())?;
        let finished = launchctl("bootstrap", OsStr::new(&domain()), Some(path.as_os_str()))?;
        if finished.succeeded() {
            Ok(Created::Active)
        } else {
            Err(exited("bootstrap", finished))
        }
    }

    fn query_entry(id: &EntryId) -> Result<EntryState, SchedulerError> {
        Ok(if exists(&agent(id)?)? {
            EntryState::Installed
        } else {
            EntryState::NotInstalled
        })
    }

    fn remove_entry(id: &EntryId) -> Result<Removal, SchedulerError> {
        let path = agent(id)?;
        if !exists(&path)? {
            return Ok(Removal::NotInstalled);
        }
        bootout(id)?;
        unix::remove_if_present(&path)
            .map_err(|source| SchedulerError::io("remove", &path, source))?;
        Ok(Removal::Removed)
    }

    fn policy_locations(identity: &PolicyIdentity) -> Result<Vec<PolicyLocation>, PolicyError> {
        Ok(profiles(identity)?
            .into_iter()
            .map(|(path, scope)| PolicyLocation::ManagedPreferences { path, scope })
            .collect())
    }

    fn read_policy(identity: &PolicyIdentity) -> Result<Vec<PolicyRead>, PolicyError> {
        let mut found = Vec::new();
        for (path, scope) in profiles(identity)? {
            // Why: macOS files a user's profile in a directory that user owns, so
            // only the machine profile can be held to root ownership.
            let found_here = match scope {
                PolicyScope::Machine => unix::read_policy_file(&path, unix::root_owned)?,
                PolicyScope::User => unix::read_policy_file(&path, |_| true)?,
            };
            if let Some(bytes) = found_here {
                found.push(PolicyRead {
                    location: PolicyLocation::ManagedPreferences { path, scope },
                    content: PolicyContent::Document(bytes),
                });
            }
        }
        Ok(found)
    }
}
