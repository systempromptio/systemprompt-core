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

pub mod settings;

use settings::{apply_managed_settings, key_helper_path, remove_managed_settings};

pub(crate) use settings::seed_default_model;

use super::MdmError;

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

fn managed_range(existing: &str) -> Option<(usize, usize)> {
    let (open, close) = markers();
    let start = existing.find(&open)?;
    let close_at = existing[start..].find(&close).map(|i| start + i)?;
    let end = existing[close_at..]
        .find('\n')
        .map_or(existing.len(), |n| close_at + n + 1);
    Some((start, end))
}

fn splice(existing: &str, block: &str) -> Option<String> {
    let replaced = if let Some((start, end)) = managed_range(existing) {
        format!("{}{block}{}", &existing[..start], &existing[end..])
    } else {
        let mut out = existing.to_owned();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(block);
        out
    };
    (replaced != existing).then_some(replaced)
}

fn io_error(action: &'static str, path: &Path) -> impl FnOnce(std::io::Error) -> MdmError {
    let path = path.to_path_buf();
    move |source| MdmError::Io {
        action,
        path,
        source,
    }
}

fn write_atomic(path: &Path, contents: &str) -> Result<(), MdmError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error("create", parent))?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&tmp, contents).map_err(io_error("write", &tmp))?;
    fs::rename(&tmp, path).map_err(|e| {
        _ = fs::remove_file(&tmp);
        io_error("rename onto", path)(e)
    })
}

fn read_or_empty(path: &Path) -> Result<String, MdmError> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(io_error("read", path)(e)),
    }
}

pub(super) fn apply(gateway: &str) -> Result<Vec<String>, MdmError> {
    let env_file = env_file_path().ok_or(MdmError::Resolve("the user's config directory"))?;
    let key_path =
        crate::proxy::secret::secret_path().ok_or(MdmError::Resolve("the loopback secret path"))?;
    write_atomic(&env_file, &env_file_body(gateway, &key_path))?;

    let mut lines = vec![format!(
        "wrote: {} (ANTHROPIC_BASE_URL, ANTHROPIC_AUTH_TOKEN)",
        env_file.display()
    )];

    let profile = profile_path().ok_or(MdmError::Resolve("the user's home directory"))?;
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

    lines.extend(apply_managed_settings(gateway, &key_path)?);
    // Why: Claude Code no longer depends on this — it reads the settings file
    // written above on every invocation, in any shell. `env.sh` remains for
    // other Anthropic-API clients (curl, SDK scripts) that read the process
    // environment, so say what it is for rather than presenting it as a step.
    lines.push(
        "Claude Code is configured and needs no further steps. env.sh additionally \
         exports these for other Anthropic-API clients; a new login shell picks it up."
            .to_owned(),
    );
    Ok(lines)
}

pub(crate) fn remove() -> Vec<String> {
    let mut lines = Vec::new();
    for path in [env_file_path(), key_helper_path()].into_iter().flatten() {
        match fs::remove_file(&path) {
            Ok(()) => lines.push(format!("removed: {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => lines.push(format!("could not remove {}: {e}", path.display())),
        }
    }
    lines.extend(remove_managed_settings());
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
