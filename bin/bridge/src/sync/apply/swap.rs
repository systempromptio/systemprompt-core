//! Promotion of a staged plugin directory over the installed one.
//!
//! The installed plugin is moved aside before the staged tree takes its
//! place, and moved back if the promotion fails, so a rename that cannot
//! complete (a cross-volume staging dir, an antivirus lock, a permission
//! change) leaves the previously installed plugin in service rather than
//! deleted. The displaced plugin's `node_modules` is carried into the
//! promoted tree only once the promotion has landed, so a restored plugin
//! keeps its packages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::{fs, io};

use super::ApplyError;

const DISPLACED_SUFFIX: &str = ".old";

pub fn promote_staged(stage: &Path, target: &Path, plugin_id: &str) -> Result<bool, ApplyError> {
    let displaced = displaced_path(target);
    remove_dir_if_present(&displaced).map_err(|source| ApplyError::Io {
        context: format!("remove leftover {}", displaced.display()),
        source,
    })?;

    let was_present = target.exists();
    if was_present {
        fs::rename(target, &displaced).map_err(|source| ApplyError::Io {
            context: format!("set aside installed {plugin_id}"),
            source,
        })?;
    }

    if let Err(source) = fs::rename(stage, target) {
        if was_present {
            restore(&displaced, target, plugin_id);
        }
        return Err(ApplyError::Io {
            context: format!("promote staged {plugin_id}"),
            source,
        });
    }

    if !target.is_dir() {
        if was_present {
            restore(&displaced, target, plugin_id);
        }
        return Err(ApplyError::Io {
            context: format!("promote staged {plugin_id}"),
            source: io::Error::other("promoted plugin directory did not land"),
        });
    }

    if was_present {
        super::node_deps::carry_over(&displaced, target);
        fs::remove_dir_all(&displaced).map_err(|source| ApplyError::Io {
            context: format!("remove displaced {}", displaced.display()),
            source,
        })?;
    }
    Ok(was_present)
}

fn restore(displaced: &Path, target: &Path, plugin_id: &str) {
    match fs::rename(displaced, target) {
        Ok(()) => tracing::warn!(
            plugin_id,
            "promotion failed; the installed plugin was restored"
        ),
        Err(error) => tracing::error!(
            plugin_id,
            error = %error,
            displaced = %displaced.display(),
            "promotion failed and the installed plugin could not be moved back"
        ),
    }
}

fn remove_dir_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn displaced_path(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_owned();
    name.push(DISPLACED_SUFFIX);
    PathBuf::from(name)
}
