//! Sandboxed Git ref resolution and regular-file tree import.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::*;

pub(super) fn resolve_ref(
    repository: &str,
    reference: &str,
    credential: Option<&str>,
) -> Result<String> {
    if is_commit(reference) {
        return Ok(reference.to_owned());
    }
    let peeled = format!("{reference}^{{}}");
    let output = git(
        Command::new("git").args([
            "-c",
            "core.hooksPath=/dev/null",
            "ls-remote",
            "--exit-code",
            repository,
            reference,
            &peeled,
        ]),
        credential,
    )?;
    let entries = std::str::from_utf8(&output).map_err(|_| ManagedError::Integrity)?;
    let line = entries
        .lines()
        .find(|line| line.split_whitespace().nth(1) == Some(peeled.as_str()))
        .or_else(|| entries.lines().next())
        .ok_or(ManagedError::Unavailable)?;
    let commit = line
        .split_whitespace()
        .next()
        .ok_or(ManagedError::Integrity)?;
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(ManagedError::Integrity);
    }
    Ok(commit.to_owned())
}

fn is_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

pub(super) fn import_tree(
    temp: &Path,
    repository: &str,
    commit: &str,
    subdirectory: Option<&str>,
    root: &str,
    credential: Option<&str>,
) -> Result<RevisionFiles> {
    git(
        Command::new("git")
            .args(["-c", "core.hooksPath=/dev/null", "init", "--bare"])
            .arg(temp),
        None,
    )?;
    git(
        Command::new("git").current_dir(temp).args([
            "-c",
            "core.hooksPath=/dev/null",
            "fetch",
            "--no-tags",
            "--depth=1",
            repository,
            commit,
        ]),
        credential,
    )?;
    let prefix = [subdirectory, Some(root)]
        .into_iter()
        .flatten()
        .collect::<PathBuf>();
    let prefix_text = prefix
        .to_str()
        .ok_or(ManagedError::Integrity)?
        .trim_matches('/');
    let listing = git(
        Command::new("git").current_dir(temp).args([
            "-c",
            "core.hooksPath=/dev/null",
            "ls-tree",
            "-rz",
            "-r",
            "--full-tree",
            commit,
            "--",
            prefix_text,
        ]),
        None,
    )?;
    let mut files = BTreeMap::new();
    for entry in listing
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let separator = entry
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or(ManagedError::Integrity)?;
        let (metadata, path_with_separator) = entry.split_at(separator);
        let path = path_with_separator
            .get(1..)
            .ok_or(ManagedError::Integrity)?;
        let metadata = std::str::from_utf8(metadata).map_err(|_| ManagedError::Integrity)?;
        let mut fields = metadata.split_whitespace();
        let mode = fields.next().ok_or(ManagedError::Integrity)?;
        if fields.next() != Some("blob") || !matches!(mode, "100644" | "100755") {
            return Err(super::super::error::invalid(
                "Git source contains links, submodules, or non-regular files",
            ));
        }
        let path = std::str::from_utf8(path)
            .map_err(|_| super::super::error::invalid("Git paths must be UTF-8"))?;
        let relative = path
            .strip_prefix(prefix_text)
            .and_then(|value| value.strip_prefix('/'))
            .ok_or(ManagedError::Integrity)?;
        super::super::assets::validate_path(relative)?;
        if files.contains_key(relative) {
            return Err(super::super::error::invalid(
                "Git source contains duplicate targets",
            ));
        }
        let object = format!("{commit}:{path}");
        let bytes = git(
            Command::new("git").current_dir(temp).args([
                "-c",
                "core.hooksPath=/dev/null",
                "cat-file",
                "blob",
                &object,
            ]),
            None,
        )?;
        files.insert(
            relative.to_owned(),
            AssetFile {
                bytes,
                media_type: "application/octet-stream".to_owned(),
                executable: mode == "100755",
            },
        );
    }
    let files = RevisionFiles(files);
    if !files.0.is_empty() {
        files.validate()?;
    }
    Ok(files)
}

fn git(command: &mut Command, credential: Option<&str>) -> Result<Vec<u8>> {
    if let Some(token) = credential {
        command
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "http.extraHeader")
            .env(
                "GIT_CONFIG_VALUE_0",
                format!("Authorization: Bearer {token}"),
            );
    }
    let output = command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()?;
    if !output.status.success() {
        return Err(ManagedError::Conflict(
            "Git synchronization failed without changing publication selection".to_owned(),
        ));
    }
    Ok(output.stdout)
}
