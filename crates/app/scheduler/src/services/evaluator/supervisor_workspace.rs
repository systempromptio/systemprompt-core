//! Evaluator workspace materialization and integrity helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    ArtifactFile, BTreeMap, BTreeSet, CaseContent, EvalExecutionId, EvaluatorSupervisorConfig,
    ExecutionEvidence, Path, PathBuf, RevisionBundle, SchedulerError, SchedulerResult,
};
use sha2::{Digest, Sha256};

#[path = "supervisor_workspace_walk.rs"]
mod walk;
use walk::visit_workspace;

pub fn install_case_fixtures(case: &CaseContent, root: &Path) -> SchedulerResult<()> {
    if case.fixtures.len() > 256
        || case.fixtures.values().map(String::len).sum::<usize>() > 8 * 1024 * 1024
    {
        return Err(SchedulerError::config_error(
            "Case fixtures exceed workspace limits",
        ));
    }
    super::ResourceContent::Case(case.clone())
        .validate()
        .map_err(internal)?;
    for (relative, content) in &case.fixtures {
        let path = root.join(relative);
        ensure_no_directory_links(&path, root)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        write_private(&path, content.as_bytes())?;
    }
    Ok(())
}

pub fn workspace_state(root: &Path) -> SchedulerResult<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    visit_workspace(root, root, &mut |relative, bytes, executable| {
        files.insert(relative, file_state_digest(bytes, executable));
        Ok(())
    })?;
    Ok(files)
}

fn file_state_digest(bytes: &[u8], executable: bool) -> String {
    let mut digest = Sha256::new();
    digest.update([u8::from(executable)]);
    digest.update(bytes);
    hex::encode(digest.finalize())
}

pub fn changed_workspace(
    root: &Path,
    baseline: &BTreeMap<String, String>,
) -> SchedulerResult<BTreeMap<String, ArtifactFile>> {
    let mut files = BTreeMap::new();
    let mut bytes_total = 0usize;
    visit_workspace(root, root, &mut |relative, bytes, executable| {
        let digest = file_state_digest(bytes, executable);
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

pub fn materialize_root(bundle: &RevisionBundle, destination: &Path) -> SchedulerResult<()> {
    bundle.verify().map_err(internal)?;
    install_files(
        &bundle.revision_files(&bundle.root).map_err(internal)?.0,
        destination,
    )
}

pub fn materialize_skills(bundle: &RevisionBundle, destination: &Path) -> SchedulerResult<()> {
    bundle.verify().map_err(internal)?;
    for revision in bundle.revisions.keys() {
        let files = bundle.revision_files(revision).map_err(internal)?;
        let config = files
            .0
            .get("config.yaml")
            .map(|file| serde_yaml::from_slice::<systemprompt_models::DiskSkillConfig>(&file.bytes))
            .transpose()
            .map_err(|error| {
                SchedulerError::config_error(format!(
                    "Managed skill {} carries a malformed config.yaml: {error}",
                    revision.as_str()
                ))
            })?;
        if let Some(config) = config {
            let id = if config.id.as_str().is_empty() {
                revision.as_str()
            } else {
                config.id.as_str()
            };
            validate_directory_component(id)?;
            let directory = destination.join(id.replace('_', "-"));
            ensure_no_directory_links(&directory, destination)?;
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
                install_file(&directory.join(path), file, &directory)?;
            }
        } else {
            validate_directory_component(revision.as_str())?;
            let directory = destination.join(revision.as_str());
            ensure_no_directory_links(&directory, destination)?;
            install_files(&files.0, &directory)?;
        }
    }
    Ok(())
}

fn validate_directory_component(value: &str) -> SchedulerResult<()> {
    if value.is_empty() || matches!(value, "." | "..") || value.contains(['/', '\\', ':']) {
        return Err(SchedulerError::config_error(
            "Skill directory must be one relative component",
        ));
    }
    Ok(())
}

fn install_files(
    files: &BTreeMap<String, systemprompt_models::managed::AssetFile>,
    destination: &Path,
) -> SchedulerResult<()> {
    for (path, file) in files {
        install_file(&destination.join(path), file, destination)?;
    }
    Ok(())
}

fn install_file(
    path: &Path,
    file: &systemprompt_models::managed::AssetFile,
    destination: &Path,
) -> SchedulerResult<()> {
    ensure_no_directory_links(path, destination)?;
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

fn ensure_no_directory_links(path: &Path, root: &Path) -> SchedulerResult<()> {
    for ancestor in path
        .ancestors()
        .take_while(|ancestor| ancestor.starts_with(root))
    {
        if std::fs::symlink_metadata(ancestor).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(SchedulerError::config_error(
                "Workspace write cannot traverse a directory link",
            ));
        }
    }
    Ok(())
}

pub(super) fn write_private(path: &Path, bytes: &[u8]) -> SchedulerResult<()> {
    use std::io::Write;
    ensure_no_directory_links(path, path.parent().unwrap_or(path))?;
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
        || systemprompt_evaluation::capabilities::proofs::ImmutableImage::parse(
            &config.client_image,
        )
        .is_err()
        || systemprompt_evaluation::capabilities::proofs::ImmutableImage::parse(&config.relay_image)
            .is_err()
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
    systemprompt_evaluation::capabilities::proofs::ImmutableImage::parse(image)
        .map(|image| image.digest().to_owned())
        .map_err(|error| SchedulerError::config_error(error.to_string()))
}
pub(super) fn internal(error: impl std::fmt::Display) -> SchedulerError {
    SchedulerError::Internal(error.to_string())
}

/// An exclusively created workspace removed when its owning execution ends.
#[derive(Debug)]
pub struct WorkspaceDirectory(PathBuf);

impl WorkspaceDirectory {
    pub fn create(path: PathBuf) -> SchedulerResult<Self> {
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for WorkspaceDirectory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            tracing::warn!(
                path = %self.0.display(),
                error = %error,
                "Evaluator workspace cleanup failed"
            );
        }
    }
}
