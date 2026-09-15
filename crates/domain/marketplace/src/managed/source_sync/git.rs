//! Sandboxed Git ref resolution and regular-file tree import.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{AssetFile, BTreeMap, Command, ManagedError, Path, PathBuf, Result, RevisionFiles};

pub(super) fn resolve_ref(
    repository: &str,
    reference: &str,
    credential: Option<&str>,
    deadline: std::time::Instant,
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
        credential.map(|token| (repository, token)),
        deadline,
    )?;
    let entries = std::str::from_utf8(&output).map_err(|_corrupt| ManagedError::Integrity)?;
    let line = entries
        .lines()
        .find(|line| line.split_whitespace().nth(1) == Some(peeled.as_str()))
        .or_else(|| entries.lines().next())
        .ok_or(ManagedError::Unavailable)?;
    let commit = line
        .split_whitespace()
        .next()
        .ok_or(ManagedError::Integrity)?;
    if !matches!(commit.len(), 40 | 64)
        || !commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(ManagedError::Integrity);
    }
    Ok(commit.to_owned())
}

pub(super) fn is_commit(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

pub(super) struct GitCheckout<'a> {
    pub temp: &'a Path,
    pub repository: &'a str,
    pub commit: &'a str,
    pub subdirectory: Option<&'a str>,
    pub root: &'a str,
    pub credential: Option<&'a str>,
    pub certificate_authority: Option<&'a [u8]>,
    pub deadline: std::time::Instant,
}

pub(super) fn import_tree(checkout: &GitCheckout<'_>) -> Result<RevisionFiles> {
    let GitCheckout {
        temp,
        commit,
        subdirectory,
        root,
        deadline,
        ..
    } = *checkout;
    fetch_commit(checkout)?;
    let prefix = [subdirectory, Some(root)]
        .into_iter()
        .flatten()
        .collect::<PathBuf>();
    let prefix_text = prefix
        .to_str()
        .ok_or(ManagedError::Integrity)?
        .trim_matches('/');
    let listing = list_tree(temp, commit, prefix_text, deadline)?;
    let mut files = BTreeMap::new();
    for entry in listing
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let (relative, mode, path) = parse_tree_entry(entry, prefix_text)?;
        if files.len() >= 256 || files.contains_key(relative) {
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
            deadline,
        )?;
        if files
            .values()
            .map(|file: &AssetFile| file.bytes.len())
            .sum::<usize>()
            + bytes.len()
            > 8 * 1024 * 1024
        {
            return Err(super::super::error::invalid("Git tree exceeds 8 MiB"));
        }
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

fn fetch_commit(checkout: &GitCheckout<'_>) -> Result<()> {
    let GitCheckout {
        temp,
        repository,
        commit,
        credential,
        certificate_authority,
        deadline,
        ..
    } = *checkout;
    git(
        Command::new("git")
            .args(["-c", "core.hooksPath=/dev/null", "init", "--bare"])
            .arg(if commit.len() == 64 {
                "--object-format=sha256"
            } else {
                "--object-format=sha1"
            })
            .arg(temp),
        None,
        deadline,
    )?;
    let mut fetch = Command::new("git");
    fetch
        .current_dir(temp)
        .args(["-c", "core.hooksPath=/dev/null"]);
    if let Some(certificate) = certificate_authority {
        use std::io::Write;
        let path = temp.join(".systemprompt-ca.pem");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(&path)?.write_all(certificate)?;
        let path = path.to_str().ok_or(ManagedError::Integrity)?;
        fetch.args(["-c", &format!("http.sslCAInfo={path}")]);
    }
    fetch.args(["fetch", "--no-tags", "--depth=1", repository, commit]);
    git(
        &mut fetch,
        credential.map(|token| (repository, token)),
        deadline,
    )?;
    let fetched = git(
        Command::new("git")
            .current_dir(temp)
            .args(["rev-list", "--max-count=1", "FETCH_HEAD"]),
        None,
        deadline,
    )?;
    if std::str::from_utf8(&fetched)
        .map_err(|_error| ManagedError::Integrity)?
        .trim()
        != commit
    {
        return Err(ManagedError::Integrity);
    }
    require_plain_tree(temp, commit, deadline)
}

fn require_plain_tree(temp: &Path, commit: &str, deadline: std::time::Instant) -> Result<()> {
    let tree = git(
        Command::new("git")
            .current_dir(temp)
            .args(["ls-tree", "-rz", "-r", "--full-tree", commit]),
        None,
        deadline,
    )?;
    for entry in tree
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let text = std::str::from_utf8(entry).map_err(|_error| ManagedError::Integrity)?;
        let path = text.split_once('\t').ok_or(ManagedError::Integrity)?.1;
        if text.starts_with("160000 ")
            || path
                .split('/')
                .any(|part| matches!(part, ".git" | ".gitmodules"))
        {
            return Err(super::super::error::invalid(
                "Repository contains undeclared submodules or nested Git metadata",
            ));
        }
    }
    Ok(())
}

fn list_tree(
    temp: &Path,
    commit: &str,
    prefix_text: &str,
    deadline: std::time::Instant,
) -> Result<Vec<u8>> {
    git(
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
        deadline,
    )
}

fn parse_tree_entry<'a>(entry: &'a [u8], prefix_text: &str) -> Result<(&'a str, &'a str, &'a str)> {
    let separator = entry
        .iter()
        .position(|byte| *byte == b'\t')
        .ok_or(ManagedError::Integrity)?;
    let (metadata, path_with_separator) = entry.split_at(separator);
    let path = path_with_separator
        .get(1..)
        .ok_or(ManagedError::Integrity)?;
    let metadata = std::str::from_utf8(metadata).map_err(|_corrupt| ManagedError::Integrity)?;
    let mut fields = metadata.split_whitespace();
    let mode = fields.next().ok_or(ManagedError::Integrity)?;
    if fields.next() != Some("blob") || !matches!(mode, "100644" | "100755") {
        return Err(super::super::error::invalid(
            "Git source contains links, submodules, or non-regular files",
        ));
    }
    let path = std::str::from_utf8(path).map_err(|error| {
        super::super::error::invalid(&format!("Git paths must be UTF-8: {error}"))
    })?;
    if path
        .split('/')
        .any(|part| matches!(part, ".git" | ".gitmodules"))
    {
        return Err(super::super::error::invalid(
            "Nested Git metadata is not permitted",
        ));
    }
    let relative = path
        .strip_prefix(prefix_text)
        .and_then(|value| value.strip_prefix('/'))
        .ok_or(ManagedError::Integrity)?;
    systemprompt_models::managed::validate_path(relative)?;
    Ok((relative, mode, path))
}

fn git(
    command: &mut Command,
    credential: Option<(&str, &str)>,
    deadline: std::time::Instant,
) -> Result<Vec<u8>> {
    let remaining = deadline
        .checked_duration_since(std::time::Instant::now())
        .ok_or(ManagedError::Unavailable)?;
    let mut limits = super::super::git_execution::GitExecutionLimits::default();
    limits.deadline = limits.deadline.min(remaining);
    super::super::git_execution::execute(command, credential, limits)
}
