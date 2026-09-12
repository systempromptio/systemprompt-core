//! Bounded authoring-tree capture. Imported files are never executed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;

use serde::Serialize;
use systemprompt_models::DiskSkillConfig;

use super::error::invalid;
use super::provenance::validate_key;
use super::{AssetDigest, AssetFile, FileEntry, ManagedError, Result, RevisionFiles};

#[derive(Debug, Clone, Serialize)]
pub struct CapturedSkills {
    pub(super) skills: BTreeMap<String, RevisionFiles>,
    pub(super) tree_digest: AssetDigest,
}

impl CapturedSkills {
    pub fn skills(&self) -> &BTreeMap<String, RevisionFiles> {
        &self.skills
    }
    pub const fn tree_digest(&self) -> &AssetDigest {
        &self.tree_digest
    }
}

pub fn capture_skills(services_root: &Path, skill_ids: &[String]) -> Result<CapturedSkills> {
    let captured = capture_once(services_root, skill_ids)?;
    if capture_once(services_root, skill_ids)?.tree_digest != captured.tree_digest {
        return Err(invalid(
            "Authoring tree changed during capture; retry from a stable tree",
        ));
    }
    Ok(captured)
}

#[derive(Default)]
struct CaptureBudget {
    bytes: usize,
    files: usize,
}

fn capture_once(services_root: &Path, skill_ids: &[String]) -> Result<CapturedSkills> {
    if skill_ids.is_empty() || skill_ids.len() > 100 {
        return Err(invalid("Expected 1–100 skill IDs"));
    }
    reject_link(services_root)?;
    let root = services_root.join("skills");
    reject_link(&root)?;
    let mut skills = BTreeMap::new();
    let mut manifests = BTreeMap::new();
    let mut budget = CaptureBudget::default();
    for id in skill_ids {
        validate_key(id)?;
        if id.contains('/') || id == "." || id == ".." || skills.contains_key(id) {
            return Err(invalid("Invalid or duplicate skill ID"));
        }
        let path = root.join(id);
        let mut files = RevisionFiles::default();
        capture_directory(&path, &path, &mut files, &mut budget)?;
        validate_skill(id, &files)?;
        manifests.insert(id.clone(), file_manifest(&files));
        skills.insert(id.clone(), files);
    }
    Ok(CapturedSkills {
        tree_digest: AssetDigest::of(&serde_jcs::to_vec(&manifests)?),
        skills,
    })
}

fn validate_skill(id: &str, files: &RevisionFiles) -> Result<()> {
    files.validate()?;
    let config = files
        .0
        .get("config.yaml")
        .ok_or_else(|| invalid("Skill configuration is missing"))?;
    let config: DiskSkillConfig = serde_yaml::from_slice(&config.bytes)
        .map_err(|_| invalid("Skill configuration is invalid"))?;
    if !config.id.as_str().is_empty() && config.id.as_str() != id {
        return Err(invalid("Skill ID does not match its authoring directory"));
    }
    if !config.enabled || !files.0.contains_key(config.content_file()) {
        return Err(invalid(
            "Baseline skills must be enabled and contain their instruction file",
        ));
    }
    Ok(())
}

fn capture_directory(
    base: &Path,
    current: &Path,
    files: &mut RevisionFiles,
    budget: &mut CaptureBudget,
) -> Result<()> {
    reject_link(current)?;
    if current
        .strip_prefix(base)
        .map_err(|_| invalid("Directory escaped root"))?
        .components()
        .count()
        > 32
    {
        return Err(invalid("Authoring tree exceeds maximum directory depth"));
    }
    for entry in fs::read_dir(current).map_err(ManagedError::Io)? {
        let entry = entry.map_err(ManagedError::Io)?;
        let path = entry.path();
        let kind = entry.file_type().map_err(ManagedError::Io)?;
        if kind.is_symlink() {
            return Err(invalid("Authoring trees cannot contain symlinks"));
        }
        if kind.is_dir() {
            capture_directory(base, &path, files, budget)?;
        } else if kind.is_file() {
            capture_file(base, &path, files, budget)?;
        } else {
            return Err(invalid(
                "Authoring trees may contain only regular files and directories",
            ));
        }
    }
    Ok(())
}

fn capture_file(
    base: &Path,
    path: &Path,
    files: &mut RevisionFiles,
    budget: &mut CaptureBudget,
) -> Result<()> {
    let relative = path
        .strip_prefix(base)
        .map_err(|_| invalid("File escaped authoring root"))?;
    let relative = relative
        .to_str()
        .ok_or_else(|| invalid("Authoring paths must be UTF-8"))?
        .replace('\\', "/");
    super::assets::validate_path(&relative)?;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(ManagedError::Io)?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(ManagedError::Io)?;
    budget.bytes = budget
        .bytes
        .checked_add(bytes.len())
        .ok_or_else(|| invalid("Authoring size overflow"))?;
    budget.files += 1;
    if budget.bytes > 8 * 1024 * 1024 || budget.files > 256 {
        return Err(invalid("Captured authoring tree exceeds workspace limits"));
    }
    let file = AssetFile {
        bytes,
        media_type: media_type(path).to_owned(),
        executable: executable(path)?,
    };
    files.0.insert(relative, file);
    Ok(())
}

fn file_manifest(files: &RevisionFiles) -> BTreeMap<String, FileEntry> {
    files
        .0
        .iter()
        .map(|(path, file)| {
            (
                path.clone(),
                FileEntry {
                    digest: AssetDigest::of(&file.bytes),
                    bytes: file.bytes.len() as u64,
                    media_type: file.media_type.clone(),
                    executable: file.executable,
                },
            )
        })
        .collect()
}

fn reject_link(path: &Path) -> Result<()> {
    if fs::symlink_metadata(path)
        .map_err(ManagedError::Io)?
        .file_type()
        .is_symlink()
    {
        return Err(invalid("Authoring roots cannot be symlinks"));
    }
    Ok(())
}

fn media_type(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("md") => "text/markdown",
        Some("yaml" | "yml") => "application/yaml",
        Some("json") => "application/json",
        Some("txt" | "sh" | "py") => "text/plain",
        _ => "application/octet-stream",
    }
}

#[cfg(unix)]
fn executable(path: &Path) -> Result<bool> {
    use std::os::unix::fs::PermissionsExt;
    Ok(fs::metadata(path)
        .map_err(ManagedError::Io)?
        .permissions()
        .mode()
        & 0o111
        != 0)
}

#[cfg(not(unix))]
fn executable(path: &Path) -> Result<bool> {
    Ok(matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("sh" | "py")
    ))
}
