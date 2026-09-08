//! macOS install swap.
//!
//! The artifact is a zipped `.app`, not the `.dmg` humans download: mounting a
//! disk image from a background process is slow and needs cleanup, whereas the
//! zip unpacks straight into a staging directory.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::update::error::UpdateError;
use crate::update::install::{probe_writable, running_exe};

pub(super) fn apply(staged: &Path) -> Result<PathBuf, UpdateError> {
    let bundle = running_bundle()?;
    probe_writable(
        &bundle,
        "move the app to /Applications and try again — it cannot update in place from a disk image",
    )?;

    let workdir = staged.with_extension("unpack");
    if workdir.exists() {
        // Why: discard-ok: temporary cleanup cannot change the installed result.
        _ = std::fs::remove_dir_all(&workdir);
    }
    std::fs::create_dir_all(&workdir).map_err(|e| UpdateError::io(&workdir, e))?;

    let result = unpack(staged, &workdir)
        .and_then(|new_bundle| {
            verify_signature(&new_bundle)?;
            Ok(new_bundle)
        })
        .and_then(|new_bundle| swap(&new_bundle, &bundle));

    if let Err(e) = std::fs::remove_dir_all(&workdir) {
        tracing::debug!(error = %e, path = %workdir.display(), "update: unpack cleanup failed");
    }
    result.map(|()| bundle)
}

fn running_bundle() -> Result<PathBuf, UpdateError> {
    let exe = running_exe()?;
    exe.ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .map(Path::to_path_buf)
        .ok_or_else(|| UpdateError::LocateInstall {
            what: "application bundle",
            detail: format!(
                "{} is not inside a .app; a bare binary must be updated with the CLI",
                exe.display()
            ),
        })
}

// Why: ditto preserves bundle symlinks and extended attributes needed for macOS
// signature validation.
fn unpack(archive: &Path, into: &Path) -> Result<PathBuf, UpdateError> {
    let out = Command::new("/usr/bin/ditto")
        .arg("-x")
        .arg("-k")
        .arg(archive)
        .arg(into)
        .output()
        .map_err(|e| UpdateError::Unpack(format!("could not run ditto: {e}")))?;
    if !out.status.success() {
        return Err(UpdateError::Unpack(format!(
            "ditto failed to expand the archive: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    std::fs::read_dir(into)
        .map_err(|e| UpdateError::io(into, e))?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .ok_or_else(|| UpdateError::Unpack("the archive contains no .app bundle".to_owned()))
}

fn verify_signature(bundle: &Path) -> Result<(), UpdateError> {
    let codesign = Command::new("/usr/bin/codesign")
        .arg("--verify")
        .arg("--deep")
        .arg("--strict")
        .arg(bundle)
        .output()
        .map_err(|e| UpdateError::Signature(format!("could not run codesign: {e}")))?;
    if !codesign.status.success() {
        return Err(UpdateError::Signature(format!(
            "the downloaded app is not validly signed: {}",
            String::from_utf8_lossy(&codesign.stderr).trim()
        )));
    }

    let spctl = Command::new("/usr/sbin/spctl")
        .arg("--assess")
        .arg("--type")
        .arg("execute")
        .arg(bundle)
        .output()
        .map_err(|e| UpdateError::Signature(format!("could not run spctl: {e}")))?;
    if !spctl.status.success() {
        return Err(UpdateError::Signature(format!(
            "the downloaded app was rejected by Gatekeeper: {}",
            String::from_utf8_lossy(&spctl.stderr).trim()
        )));
    }

    tracing::info!(path = %bundle.display(), "update: signature and Gatekeeper assessment passed");
    Ok(())
}

fn swap(new_bundle: &Path, target: &Path) -> Result<(), UpdateError> {
    let backup = crate::fsutil::temp_path_for(target);
    std::fs::rename(target, &backup).map_err(|e| UpdateError::io(target, e))?;

    let out = Command::new("/usr/bin/ditto")
        .arg(new_bundle)
        .arg(target)
        .output();

    let copied = match out {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(UpdateError::Unpack(format!(
            "ditto failed to install the new bundle: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ))),
        Err(e) => Err(UpdateError::Unpack(format!("could not run ditto: {e}"))),
    };

    match copied {
        Ok(()) => {
            if let Err(e) = std::fs::remove_dir_all(&backup) {
                tracing::debug!(error = %e, path = %backup.display(), "update: old bundle cleanup failed");
            }
            tracing::info!(path = %target.display(), "update: bundle replaced");
            Ok(())
        },
        Err(e) => {
            match std::fs::remove_dir_all(target) {
                Ok(()) => {},
                Err(cleanup) if cleanup.kind() == std::io::ErrorKind::NotFound => {},
                Err(cleanup) => {
                    return Err(UpdateError::Unpack(format!(
                        "{e}; rollback cannot remove {}: {cleanup}; previous app remains at {}",
                        target.display(),
                        backup.display()
                    )));
                },
            }
            if let Err(restore) = std::fs::rename(&backup, target) {
                return Err(UpdateError::Unpack(format!(
                    "{e}; rollback failed: {restore}; previous app remains at {}",
                    backup.display()
                )));
            }
            Err(e)
        },
    }
}
