//! Evaluator workspace materialization and integrity helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::*;

pub(super) fn install_case_fixtures(case: &CaseContent, root: &Path) -> SchedulerResult<()> {
    for (relative, content) in &case.fixtures {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        write_private(&path, content.as_bytes())?;
    }
    Ok(())
}

pub(super) fn workspace_state(root: &Path) -> SchedulerResult<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    visit_workspace(root, root, &mut |relative, bytes, _| {
        files.insert(relative, hex::encode(Sha256::digest(bytes)));
        Ok(())
    })?;
    Ok(files)
}

pub(super) fn changed_workspace(
    root: &Path,
    baseline: &BTreeMap<String, String>,
) -> SchedulerResult<BTreeMap<String, ArtifactFile>> {
    let mut files = BTreeMap::new();
    let mut bytes_total = 0usize;
    visit_workspace(root, root, &mut |relative, bytes, executable| {
        let digest = hex::encode(Sha256::digest(bytes));
        if baseline.get(&relative) != Some(&digest) {
            bytes_total = bytes_total
                .checked_add(bytes.len())
                .ok_or_else(|| SchedulerError::config_error("Workspace evidence size overflow"))?;
            if files.len() >= 252 || bytes_total > 15 * 1024 * 1024 {
                return Err(SchedulerError::config_error(
                    "Workspace evidence exceeds retained limits",
                ));
            }
            files.insert(
                format!("workspace/{relative}"),
                ArtifactFile {
                    bytes: bytes.to_vec(),
                    executable,
                },
            );
        }
        Ok(())
    })?;
    Ok(files)
}

fn visit_workspace(
    root: &Path,
    directory: &Path,
    visitor: &mut impl FnMut(String, &[u8], bool) -> SchedulerResult<()>,
) -> SchedulerResult<()> {
    let mut entries = std::fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(SchedulerError::config_error(
                "Evaluation workspace contains a link",
            ));
        }
        if metadata.is_dir() {
            visit_workspace(root, &path, visitor)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(internal)?
                .to_string_lossy()
                .replace('\\', "/");
            #[cfg(unix)]
            let executable = {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o111 != 0
            };
            #[cfg(not(unix))]
            let executable = false;
            visitor(relative, &std::fs::read(path)?, executable)?;
        } else {
            return Err(SchedulerError::config_error(
                "Evaluation workspace contains a non-regular file",
            ));
        }
    }
    Ok(())
}

pub(super) fn evidence_references(evidence: &ExecutionEvidence) -> BTreeSet<String> {
    let mut references = BTreeSet::new();
    for artifact in &evidence.artifacts {
        references.insert(artifact.relative_path.clone());
        references.insert(artifact.sha256.clone());
    }
    for request in &evidence.requests {
        references.insert(request.as_str().to_owned());
    }
    references
}

fn decoded_bundle(value: &serde_json::Value) -> SchedulerResult<RevisionBundle> {
    let bundle: RevisionBundle = serde_json::from_value(value.clone())
        .map_err(|error| SchedulerError::Internal(error.to_string()))?;
    bundle.verify().map_err(internal)?;
    Ok(bundle)
}

pub(super) fn materialize_root(
    value: &serde_json::Value,
    destination: &Path,
) -> SchedulerResult<()> {
    let bundle = decoded_bundle(value)?;
    install_files(
        &bundle.revision_files(&bundle.root).map_err(internal)?.0,
        destination,
    )
}

pub(super) fn materialize_skills(
    value: &serde_json::Value,
    destination: &Path,
) -> SchedulerResult<()> {
    let bundle = decoded_bundle(value)?;
    for revision in bundle.revisions.keys() {
        let files = bundle.revision_files(revision).map_err(internal)?;
        let config = files.0.get("config.yaml").and_then(|file| {
            serde_yaml::from_slice::<systemprompt_models::DiskSkillConfig>(&file.bytes).ok()
        });
        if let Some(config) = config {
            let id = if config.id.as_str().is_empty() {
                revision.as_str()
            } else {
                config.id.as_str()
            };
            let directory = destination.join(id.replace('_', "-"));
            std::fs::create_dir_all(&directory)?;
            let content = files
                .0
                .get(config.content_file())
                .ok_or_else(|| SchedulerError::config_error("Managed skill content is missing"))?;
            let body = std::str::from_utf8(&content.bytes)
                .map_err(|error| SchedulerError::Internal(error.to_string()))?;
            let skill_md = format!(
                "---\nname: {}\ndescription: {:?}\n---\n\n{}",
                id.replace('_', "-"),
                config.description,
                systemprompt_models::strip_frontmatter(body)
            );
            write_private(&directory.join("SKILL.md"), skill_md.as_bytes())?;
            for (path, file) in files.0.iter().filter(|(path, _)| {
                path.as_str() != "config.yaml" && path.as_str() != config.content_file()
            }) {
                install_file(&directory.join(path), file)?;
            }
        } else {
            install_files(&files.0, &destination.join(revision.as_str()))?;
        }
    }
    Ok(())
}

fn install_files(
    files: &BTreeMap<String, systemprompt_marketplace::managed::AssetFile>,
    destination: &Path,
) -> SchedulerResult<()> {
    for (path, file) in files {
        install_file(&destination.join(path), file)?;
    }
    Ok(())
}

fn install_file(
    path: &Path,
    file: &systemprompt_marketplace::managed::AssetFile,
) -> SchedulerResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_private(path, &file.bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            path,
            std::fs::Permissions::from_mode(if file.executable { 0o700 } else { 0o600 }),
        )?;
    }
    Ok(())
}

pub(super) fn write_private(path: &Path, bytes: &[u8]) -> SchedulerResult<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)?;
    Ok(())
}

pub(super) fn validate_config(config: &EvaluatorSupervisorConfig) -> SchedulerResult<()> {
    if !config.docker.is_absolute()
        || !config.workspace_root.is_absolute()
        || config.environment.trim().is_empty()
        || !config.client_image.contains("@sha256:")
        || !config.relay_image.contains("@sha256:")
        || matches!(
            config.relay_control_network.as_str(),
            "host" | "bridge" | "default" | "none"
        )
        || !matches!(config.relay_upstream.as_str(), value if value.starts_with("http://") || value.starts_with("https://"))
    {
        return Err(SchedulerError::config_error(
            "Evaluator supervisor requires absolute paths, pinned images, dedicated relay network and HTTP(S) upstream",
        ));
    }
    Ok(())
}

pub(super) fn safe_suffix(execution: &EvalExecutionId) -> String {
    execution
        .as_str()
        .bytes()
        .filter(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        .map(char::from)
        .take(48)
        .collect()
}
pub(super) fn image_digest(image: &str) -> SchedulerResult<String> {
    image
        .rsplit_once("@sha256:")
        .map(|(_, digest)| digest.to_owned())
        .ok_or_else(|| SchedulerError::config_error("Pinned image digest missing"))
}
pub(super) fn internal(error: impl std::fmt::Display) -> SchedulerError {
    SchedulerError::Internal(error.to_string())
}

#[derive(Debug)]
pub(super) struct WorkspaceDirectory(pub(super) PathBuf);

impl Drop for WorkspaceDirectory {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.0));
    }
}
