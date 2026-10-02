//! Device-authenticated consumer evidence and correctable attribution.
//!
//! A plan omits the default dev-only files (see [`crate::dev_files`]) from
//! its runtime files, the `.systemprompt-source/` copy included, and from the
//! canonical files read back against that copy, so a revision captured before
//! those excludes existed still ships a clean tree and still verifies.
//!
//! The rendered Claude Code and Claude Desktop `SKILL.md` carries the authored
//! pass-through frontmatter after `name` and `description`; the other hosts'
//! skill formats keep the two keys alone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::dev_files::DevFileFilter;
use crate::managed::error::integrity;
use crate::managed::{ManagedError, Result, RevisionBundle};
use systemprompt_identifiers::{ManagedResourceId, PublicationId};
use systemprompt_models::feedback::receipts::{
    ConsumerInstallationPlan, FileReadback, InstallationPlanFile, ReadbackStatus,
};
use systemprompt_models::feedback::{ContentDigest, EvaluatorClient};
use systemprompt_models::services::skill_frontmatter::{
    authored_skill_frontmatter, render_passthrough_frontmatter, split_skill_frontmatter,
};

pub(super) fn ships(path: &str) -> bool {
    !DevFileFilter::defaults().excludes(path, None, false)
}

pub(super) fn runtime_files(
    bundle: &RevisionBundle,
    host: EvaluatorClient,
    key: &str,
) -> Result<Vec<InstallationPlanFile>> {
    let mut files = Vec::new();
    for (revision, manifest) in &bundle.revisions {
        systemprompt_models::feedback::validate_relative_path(revision.as_str())
            .map_err(integrity)?;
        for (path, entry) in &manifest.files {
            if !ships(path) {
                continue;
            }
            let bytes = bundle
                .assets
                .get(&entry.digest)
                .ok_or(ManagedError::Integrity)?;
            files.push(InstallationPlanFile {
                path: format!(".systemprompt-source/{revision}/{path}"),
                bytes: bytes.clone(),
                executable: entry.executable,
            });
            let runtime_path = if revision == &bundle.root {
                let metadata = path == "SKILL.md";
                if metadata {
                    continue;
                }
                path.clone()
            } else {
                format!(".systemprompt-dependencies/{revision}/{path}")
            };
            files.push(InstallationPlanFile {
                path: runtime_path,
                bytes: bytes.clone(),
                executable: entry.executable,
            });
        }
    }
    files.push(InstallationPlanFile {
        path: "SKILL.md".to_owned(),
        bytes: render_skill(bundle, host, key)?.into_bytes(),
        executable: false,
    });
    files.sort_by(|a, b| a.path.cmp(&b.path));
    if files.windows(2).any(|pair| pair[0].path == pair[1].path) {
        return Err(ManagedError::Integrity);
    }
    Ok(files)
}

fn render_skill(bundle: &RevisionBundle, host: EvaluatorClient, key: &str) -> Result<String> {
    let root = bundle.revision_files(&bundle.root)?;
    let (name, description, raw, frontmatter) = if let Some(config) = root.0.get("config.yaml") {
        let config: systemprompt_models::DiskSkillConfig =
            serde_yaml::from_slice(&config.bytes).map_err(integrity)?;
        let content = root
            .0
            .get(config.content_file())
            .ok_or(ManagedError::Integrity)?;
        let raw = std::str::from_utf8(&content.bytes).map_err(integrity)?;
        let name = if config.name.is_empty() {
            key.to_owned()
        } else {
            config.name.clone()
        };
        (name, config.description, raw, config.frontmatter)
    } else {
        let content = root.0.get("SKILL.md").ok_or(ManagedError::Integrity)?;
        let raw = std::str::from_utf8(&content.bytes).map_err(integrity)?;
        let authored = authored_skill_frontmatter(raw).map_err(integrity)?;
        (
            key.to_owned(),
            String::new(),
            raw,
            split_skill_frontmatter(authored).passthrough,
        )
    };
    let instructions = systemprompt_models::strip_frontmatter(raw);
    let (rendered_name, passthrough) = match host {
        EvaluatorClient::ClaudeCode | EvaluatorClient::ClaudeDesktop => (
            key.replace('_', "-"),
            render_passthrough_frontmatter(frontmatter.as_ref()).map_err(integrity)?,
        ),
        EvaluatorClient::OpenCode => (kebab_dir(key), String::new()),
        EvaluatorClient::Codex | EvaluatorClient::Hermes => (name, String::new()),
    };
    let rendered_name = serde_json::to_string(&rendered_name).map_err(integrity)?;
    let description = serde_json::to_string(&description).map_err(integrity)?;
    let instructions = match host {
        EvaluatorClient::ClaudeCode | EvaluatorClient::ClaudeDesktop => instructions.trim(),
        _ => instructions.trim_end(),
    };
    Ok(format!(
        "---\nname: {rendered_name}\ndescription: {description}\n{passthrough}---\n\n{instructions}\n"
    ))
}

pub(super) struct PlanIdentity {
    pub(super) publication_id: PublicationId,
    pub(super) resource_id: ManagedResourceId,
    pub(super) generation: i64,
    pub(super) host: EvaluatorClient,
}

pub(super) fn build_plan(
    bundle: &RevisionBundle,
    identity: PlanIdentity,
    key: &str,
) -> Result<ConsumerInstallationPlan> {
    let PlanIdentity {
        publication_id,
        resource_id,
        generation,
        host,
    } = identity;
    bundle.verify()?;
    let canonical_files = bundle
        .revisions
        .iter()
        .flat_map(|(revision, manifest)| {
            manifest
                .files
                .iter()
                .filter(|(path, _)| ships(path))
                .map(move |(path, file)| FileReadback {
                    revision_id: revision.clone(),
                    path: path.clone(),
                    digest: ContentDigest::of(&bundle.assets[&file.digest]),
                    bytes: file.bytes,
                    executable: file.executable,
                    content_check: ReadbackStatus::Unavailable,
                    mode_check: ReadbackStatus::Unavailable,
                })
        })
        .collect();
    Ok(ConsumerInstallationPlan {
        publication_id,
        resource_id,
        revision_id: bundle.root.clone(),
        generation,
        bundle_digest: ContentDigest::try_from(bundle.digest()?.as_str().to_owned())
            .map_err(integrity)?,
        host,
        canonical_files,
        runtime_files: runtime_files(bundle, host, key)?,
    })
}

fn kebab_dir(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    let mut last_dash = true;
    for c in key.chars() {
        let mapped = if c.is_ascii_alphanumeric() {
            Some(c.to_ascii_lowercase())
        } else if last_dash {
            None
        } else {
            Some('-')
        };
        if let Some(m) = mapped {
            last_dash = m == '-';
            out.push(m);
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out.truncate(64);
    while out.ends_with('-') {
        out.pop();
    }
    out
}
