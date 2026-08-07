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

// Why: searches for the `.app` extension rather than counting path components,
// so it stays correct if the bundle layout ever gains a level.
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

// Why: `ditto -x -k` rather than an unzip crate — it preserves the symlinks,
// code signature, and extended attributes a bundle's signature covers. A naive
// unzip strips them and the result fails Gatekeeper.
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

// Why: refuses anything Gatekeeper would refuse, before a working install is
// replaced. `--deep --strict` covers nested code, and the `spctl` assessment is
// what actually decides whether the swapped-in app will launch.
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

// Why: the old bundle moves aside before the new one is written, so a failure
// mid-copy can restore the working app instead of leaving a half-written
// bundle.
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
            _ = std::fs::remove_dir_all(target);
            if let Err(restore) = std::fs::rename(&backup, target) {
                tracing::error!(
                    error = %restore,
                    backup = %backup.display(),
                    "update: install failed AND rollback failed; the previous app is at the backup path"
                );
            }
            Err(e)
        },
    }
}
