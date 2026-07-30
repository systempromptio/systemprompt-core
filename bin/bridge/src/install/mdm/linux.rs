//! Linux env configuration: a bridge-owned `env.sh` plus a marker-delimited
//! block in `~/.profile` that sources it.
//!
//! Anthropic documents no MDM channel for Linux, so the equivalent of applying
//! policy is writing the environment a login shell needs. `env.sh` is entirely
//! bridge-owned and rewritten wholesale; `~/.profile` belongs to the user, so
//! only the region between the markers is ever touched, and the rewrite goes
//! through a temp file + rename so a crash cannot truncate it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

fn markers() -> (String, String) {
    let bin = crate::brand::brand().binary_name;
    (
        format!("# >>> {bin} managed block >>>"),
        format!("# <<< {bin} managed block <<<"),
    )
}

pub(super) fn env_file_path() -> Option<PathBuf> {
    Some(
        crate::basedirs::config_dir()?
            .join(crate::brand::brand().config_dir)
            .join("env.sh"),
    )
}

fn profile_path() -> Option<PathBuf> {
    Some(crate::basedirs::home_dir()?.join(".profile"))
}

/// The token is read from the loopback key file at eval time rather than baked
/// in, so a rotated secret needs no rewrite and a missing one degrades to an
/// unset variable instead of an invalid credential.
fn env_file_body(gateway: &str, key_path: &Path) -> String {
    let bin = crate::brand::brand().binary_name;
    format!(
        "# Written by `{bin} install --apply`. Rewritten on every apply — do not edit.\n\
         export ANTHROPIC_BASE_URL=\"{gateway}\"\n\
         if [ -r \"{key}\" ]; then\n    \
             ANTHROPIC_AUTH_TOKEN=\"$(cat \"{key}\")\"\n    \
             export ANTHROPIC_AUTH_TOKEN\n\
         fi\n",
        key = key_path.display(),
    )
}

fn profile_block(env_file: &Path) -> String {
    let (open, close) = markers();
    format!(
        "{open}\n[ -r \"{path}\" ] && . \"{path}\"\n{close}\n",
        path = env_file.display(),
    )
}

/// The half-open byte range the managed block occupies, if it is present.
fn managed_range(existing: &str) -> Option<(usize, usize)> {
    let (open, close) = markers();
    let start = existing.find(&open)?;
    let close_at = existing[start..].find(&close).map(|i| start + i)?;
    let end = existing[close_at..]
        .find('\n')
        .map_or(existing.len(), |n| close_at + n + 1);
    Some((start, end))
}

/// Replaces the managed region if present, appends it otherwise — so running
/// install twice leaves exactly one block. `None` means "no change needed".
fn splice(existing: &str, block: &str) -> Option<String> {
    let replaced = match managed_range(existing) {
        Some((start, end)) => format!("{}{block}{}", &existing[..start], &existing[end..]),
        None => {
            let mut out = existing.to_owned();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(block);
            out
        },
    };
    (replaced != existing).then_some(replaced)
}

fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&tmp, contents).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| {
        _ = fs::remove_file(&tmp);
        format!("rename onto {}: {e}", path.display())
    })
}

fn read_or_empty(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

pub(super) fn apply(gateway: &str) -> Result<Vec<String>, String> {
    let env_file =
        env_file_path().ok_or_else(|| "cannot resolve the user's config directory".to_owned())?;
    let key_path = crate::proxy::secret::secret_path()
        .ok_or_else(|| "cannot resolve the loopback secret path".to_owned())?;
    write_atomic(&env_file, &env_file_body(gateway, &key_path))?;

    let mut lines = vec![format!(
        "wrote: {} (ANTHROPIC_BASE_URL, ANTHROPIC_AUTH_TOKEN)",
        env_file.display()
    )];

    let profile =
        profile_path().ok_or_else(|| "cannot resolve the user's home directory".to_owned())?;
    let existing = read_or_empty(&profile)?;
    match splice(&existing, &profile_block(&env_file)) {
        Some(updated) => {
            write_atomic(&profile, &updated)?;
            lines.push(format!("wrote: {} (managed block)", profile.display()));
        },
        None => lines.push(format!(
            "{}: managed block already current",
            profile.display()
        )),
    }
    lines.push("open a new login shell (or `. ~/.profile`) to pick these up".to_owned());
    Ok(lines)
}

/// Inverse of [`apply`]: drops the env file and the managed block, leaving the
/// rest of `~/.profile` untouched.
pub(crate) fn remove() -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(env_file) = env_file_path() {
        match fs::remove_file(&env_file) {
            Ok(()) => lines.push(format!("removed: {}", env_file.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => lines.push(format!("could not remove {}: {e}", env_file.display())),
        }
    }
    let Some(profile) = profile_path() else {
        return lines;
    };
    let Ok(existing) = read_or_empty(&profile) else {
        return lines;
    };
    if let Some((start, end)) = managed_range(&existing) {
        let stripped = format!("{}{}", &existing[..start], &existing[end..]);
        match write_atomic(&profile, &stripped) {
            Ok(()) => lines.push(format!("removed: managed block in {}", profile.display())),
            Err(e) => lines.push(format!("could not clean {}: {e}", profile.display())),
        }
    }
    lines
}
