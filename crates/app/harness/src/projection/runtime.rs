//! A skill's runtime files: the files of its publication bundle written beside
//! the projected `SKILL.md`, read back for the receipt.
//!
//! Only a skill the applier wrote (listed in the skills ledger, its `SKILL.md`
//! present) is written into. A plan is refused without `SKILL.md`, with no
//! files or too many, with a path listed twice, with a path that is not a plain
//! relative one, or past 24 MiB. Files go through a temporary file and a
//! rename, never through a link. `.systemprompt-runtime.json` (owner-only,
//! compact JSON) lists them, and what the previous plan listed and this one
//! does not is removed. Every file is read back; the plan's own `SKILL.md` too,
//! after which the projected one is put back, since the projection owns that
//! file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use sp_protocol::names::is_plain_relative_path;
use sp_protocol::wire::consumer::{Check, InstallationPlan, RuntimeReadback};
use sp_secure::{Access, contained_path, read_optional, remove_if_exists, write_if_changed};

use crate::apply::Applier;
use crate::error::MaterializeError;
use crate::ledger::{self, RUNTIME_LEDGER, SKILL_FILE, SKILLS_DIR, SKILLS_LEDGER};
use crate::readback::read_back;

const PLAN_FILES_MAX: usize = 8192;
const PLAN_BYTES_MAX: usize = 24 * 1024 * 1024;
const PATH_MAX: usize = 4096;
const SIDECAR_BYTES_MAX: usize = 1024 * 1024;

impl Applier {
    pub fn materialize(
        &self,
        skill_dir: &str,
        plan: &InstallationPlan,
    ) -> Result<Vec<RuntimeReadback>, MaterializeError> {
        let root = self.managed_skill_root(skill_dir)?;
        validate_plan(plan)?;
        let sidecar = root.join(RUNTIME_LEDGER);
        let previous = read_sidecar(&sidecar)?;
        let targets = plan
            .runtime_files
            .iter()
            .map(|file| contained(&root, &file.path).map(|path| (file, path)))
            .collect::<Result<Vec<_>, _>>()?;
        let stale = previous
            .iter()
            .filter(|path| !plan.runtime_files.iter().any(|file| &file.path == *path))
            .map(|path| contained(&root, path))
            .collect::<Result<Vec<_>, _>>()?;
        let skill_md = root.join(SKILL_FILE);
        let projected = read_optional(&skill_md)
            .map_err(MaterializeError::secure("read the projected SKILL.md"))?
            .ok_or_else(|| MaterializeError::NotManaged {
                dir: skill_dir.to_owned(),
            })?;
        for (file, path) in &targets {
            let outcome = write_if_changed(path, &file.bytes, Access::Shared)
                .map_err(MaterializeError::secure("write a runtime file"))?;
            #[cfg(unix)]
            crate::readback::set_mode(path, file.executable)?;
            tracing::debug!(skill = skill_dir, file = %file.path, ?outcome, "runtime file");
        }
        for path in &stale {
            remove_if_exists(path).map_err(MaterializeError::secure("remove a runtime file"))?;
        }
        let listed: Vec<&str> = plan
            .runtime_files
            .iter()
            .map(|file| file.path.as_str())
            .collect();
        let bytes = serde_json::to_vec(&listed).map_err(MaterializeError::Encode)?;
        let outcome = write_if_changed(&sidecar, &bytes, Access::Private)
            .map_err(MaterializeError::secure("write the runtime ledger"))?;
        tracing::debug!(skill = skill_dir, ?outcome, "runtime ledger");
        let readback = targets
            .iter()
            .map(|(file, path)| read_back(path, file))
            .collect::<Result<Vec<_>, _>>()?;
        let outcome = write_if_changed(&skill_md, &projected, Access::Shared)
            .map_err(MaterializeError::secure("restore the projected SKILL.md"))?;
        tracing::info!(
            skill = skill_dir,
            files = readback.len(),
            removed = stale.len(),
            restored = ?outcome,
            "runtime files materialized"
        );
        Ok(readback)
    }

    #[must_use]
    pub fn inspect(&self, skill_dir: &str, plan: &InstallationPlan) -> bool {
        let Ok(root) = self.managed_skill_root(skill_dir) else {
            return false;
        };
        plan.runtime_files
            .iter()
            .filter(|file| file.path != SKILL_FILE)
            .all(|file| {
                let Ok(path) = contained(&root, &file.path) else {
                    return false;
                };
                read_back(&path, file).is_ok_and(|read| {
                    read.content_check == Check::Verified && read.mode_check != Check::Mismatch
                })
            })
    }

    fn managed_skill_root(&self, skill_dir: &str) -> Result<PathBuf, MaterializeError> {
        let not_managed = || MaterializeError::NotManaged {
            dir: skill_dir.to_owned(),
        };
        let skills_root = self.config_dir().join(SKILLS_DIR);
        let ids = ledger::read_skill_ids(&skills_root.join(SKILLS_LEDGER))?;
        if !ids.iter().any(|id| id == skill_dir) {
            return Err(not_managed());
        }
        let root = contained_path(&skills_root, skill_dir).map_err(|_unsafe| not_managed())?;
        let is_dir = fs::symlink_metadata(&root).is_ok_and(|metadata| metadata.is_dir());
        let has_skill =
            fs::symlink_metadata(root.join(SKILL_FILE)).is_ok_and(|metadata| metadata.is_file());
        if is_dir && has_skill {
            Ok(root)
        } else {
            Err(not_managed())
        }
    }
}

fn validate_plan(plan: &InstallationPlan) -> Result<(), MaterializeError> {
    let invalid = |reason: &'static str| MaterializeError::InvalidPlan { reason };
    let files = &plan.runtime_files;
    if files.is_empty() || files.len() > PLAN_FILES_MAX {
        return Err(invalid("it has no files or too many"));
    }
    let mut seen = BTreeSet::new();
    let mut total = 0usize;
    for file in files {
        if !is_runtime_path(&file.path) {
            return Err(invalid("a path is not a plain relative path"));
        }
        if file.path == RUNTIME_LEDGER {
            return Err(invalid("a path names the runtime ledger"));
        }
        if !seen.insert(file.path.as_str()) {
            return Err(invalid("a path is listed twice"));
        }
        total = total.saturating_add(file.bytes.len());
        if total > PLAN_BYTES_MAX {
            return Err(invalid("its files are larger than 24 MiB"));
        }
    }
    if !seen.contains(SKILL_FILE) {
        return Err(invalid("it has no SKILL.md"));
    }
    Ok(())
}

fn is_runtime_path(path: &str) -> bool {
    path.len() <= PATH_MAX && !path.contains([':', '\0']) && is_plain_relative_path(path)
}

fn read_sidecar(path: &Path) -> Result<Vec<String>, MaterializeError> {
    let Some(bytes) =
        read_optional(path).map_err(MaterializeError::secure("read the runtime ledger"))?
    else {
        return Ok(Vec::new());
    };
    if bytes.len() > SIDECAR_BYTES_MAX {
        return Err(MaterializeError::Sidecar {
            path: path.to_path_buf(),
            reason: "it is larger than 1 MiB",
        });
    }
    let paths: Vec<String> =
        serde_json::from_slice(&bytes).map_err(|source| MaterializeError::SidecarJson {
            path: path.to_path_buf(),
            source,
        })?;
    if !paths.iter().all(|entry| is_runtime_path(entry)) {
        return Err(MaterializeError::Sidecar {
            path: path.to_path_buf(),
            reason: "it lists a path outside the skill",
        });
    }
    Ok(paths)
}

fn contained(root: &Path, relative: &str) -> Result<PathBuf, MaterializeError> {
    contained_path(root, relative).map_err(MaterializeError::secure("resolve a runtime file path"))
}
