//! Restore bootstrap ownership to the invoking sudo user and verify it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

#[cfg(target_os = "macos")]
pub(super) const CHOWN: &str = "/usr/sbin/chown";
#[cfg(not(target_os = "macos"))]
pub(super) const CHOWN: &str = "/bin/chown";

pub(super) fn chown_to_sudo_user_if_root(path: &Path) -> io::Result<()> {
    let user = match std::env::var("SUDO_USER") {
        Ok(user) if !user.is_empty() && user != "root" => user,
        Ok(_) | Err(std::env::VarError::NotPresent) => return Ok(()),
        Err(e) => return Err(io::Error::other(e)),
    };
    let uid = lookup_id(&user, "-u")?;
    let gid = lookup_id(&user, "-g")?;
    let metadata = std::fs::metadata(path)?;
    if metadata.uid() == uid && metadata.gid() == gid && tree_owned(path, uid, gid)? {
        return Ok(());
    }
    // Why: root created the whole tree, not just the root directory; a
    // non-recursive chown leaves every child unwritable for the user.
    let status = std::process::Command::new(CHOWN)
        .arg("-R")
        .arg(format!("{uid}:{gid}"))
        .arg(path)
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "chown {} for {user} exited {status}",
            path.display()
        )));
    }
    let actual = std::fs::metadata(path)?;
    if actual.uid() != uid || actual.gid() != gid || !tree_owned(path, uid, gid)? {
        return Err(io::Error::other(format!(
            "{}: ownership verification failed",
            path.display()
        )));
    }
    Ok(())
}

const OWNERSHIP_WALK_LIMIT: usize = 4096;

// Why: `chown -R` is verified on the whole tree it was asked to change, not
// one sampled entry; the walk is bounded so a runaway plugin tree cannot
// turn the check into the slow path.
fn tree_owned(path: &Path, uid: u32, gid: u32) -> io::Result<bool> {
    let mut pending = vec![path.to_path_buf()];
    let mut visited = 0usize;
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            if meta.uid() != uid || meta.gid() != gid {
                return Ok(false);
            }
            visited += 1;
            if visited >= OWNERSHIP_WALK_LIMIT {
                return Ok(true);
            }
            if meta.is_dir() {
                pending.push(entry.path());
            }
        }
    }
    Ok(true)
}
fn lookup_id(user: &str, option: &str) -> io::Result<u32> {
    let output = std::process::Command::new("/usr/bin/id")
        .arg(option)
        .arg(user)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "id {option} {user} exited {}",
            output.status
        )));
    }
    std::str::from_utf8(&output.stdout)
        .map_err(io::Error::other)?
        .trim()
        .parse()
        .map_err(io::Error::other)
}
