//! Restore bootstrap ownership to the invoking sudo user and verify it.

use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub(super) fn chown_to_sudo_user_if_root(path: &Path) -> io::Result<()> {
    let user = match std::env::var("SUDO_USER") {
        Ok(user) if !user.is_empty() && user != "root" => user,
        Ok(_) | Err(std::env::VarError::NotPresent) => return Ok(()),
        Err(e) => return Err(io::Error::other(e)),
    };
    let uid = lookup_id(&user, "-u")?;
    let gid = lookup_id(&user, "-g")?;
    let metadata = std::fs::metadata(path)?;
    if metadata.uid() == uid && metadata.gid() == gid {
        return Ok(());
    }
    let status = std::process::Command::new("chown")
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
    if actual.uid() != uid || actual.gid() != gid {
        return Err(io::Error::other(format!(
            "{}: ownership verification failed",
            path.display()
        )));
    }
    Ok(())
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
