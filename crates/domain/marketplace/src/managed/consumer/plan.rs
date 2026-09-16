//! Device-authenticated consumer evidence and correctable attribution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::credentials;
use crate::managed::error::integrity;
use crate::managed::{ManagedError, ManagedRepository, Result, RevisionBundle};
use systemprompt_identifiers::{ManagedResourceId, PublicationId, UserId};
use systemprompt_models::feedback::receipts::{
    ConsumerInstallationPlan, FileReadback, InstallationPlanFile, ReadbackStatus,
};
use systemprompt_models::feedback::{ContentDigest, EvaluatorClient};

impl ManagedRepository {
    pub async fn consumer_installation_plan(
        &self,
        credential: &str,
        resource: &ManagedResourceId,
        publication: &PublicationId,
        host: EvaluatorClient,
    ) -> Result<ConsumerInstallationPlan> {
        self.authenticate_consumer_device(credential).await?;
        let row = sqlx::query!("SELECT owner_id,revision_id,generation,bundle_digest FROM managed_publications WHERE id=$1 AND resource_id=$2 AND revision_id IS NOT NULL", publication.as_str(), resource.as_str()).fetch_optional(&self.pool).await?.ok_or(ManagedError::Unavailable)?;
        let owner = UserId::new(row.owner_id);
        let revision = systemprompt_identifiers::ResourceRevisionId::new(
            row.revision_id.ok_or(ManagedError::Integrity)?,
        );
        let bundle = self.get_revision_bundle(&owner, &revision).await?;
        let key = sqlx::query_scalar!(
            "SELECT resource_key FROM managed_resources WHERE id=$1",
            resource.as_str()
        )
        .fetch_one(&self.pool)
        .await?;
        let plan = build_plan(
            &bundle,
            PlanIdentity {
                publication_id: publication.clone(),
                resource_id: resource.clone(),
                generation: row.generation,
                host,
            },
            &key,
        )?;
        if Some(plan.bundle_digest.as_str()) != row.bundle_digest.as_deref() {
            return Err(ManagedError::Integrity);
        }
        let mut tx = self.pool.begin().await?;
        let identity = credentials::authenticate(&mut tx, credential).await?;
        credentials::require_grant(&mut tx, &owner, resource, &identity.consumer_id).await?;
        tx.commit().await?;
        Ok(plan)
    }
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
    let (name, description, instructions) = if let Some(config) = root.0.get("config.yaml") {
        let config: systemprompt_models::DiskSkillConfig =
            serde_yaml::from_slice(&config.bytes).map_err(integrity)?;
        let content = root
            .0
            .get(config.content_file())
            .ok_or(ManagedError::Integrity)?;
        let raw = std::str::from_utf8(&content.bytes).map_err(integrity)?;
        (
            if config.name.is_empty() {
                key.to_owned()
            } else {
                config.name.clone()
            },
            config.description,
            systemprompt_models::strip_frontmatter(raw),
        )
    } else {
        let content = root.0.get("SKILL.md").ok_or(ManagedError::Integrity)?;
        let raw = std::str::from_utf8(&content.bytes).map_err(integrity)?;
        (
            key.to_owned(),
            String::new(),
            systemprompt_models::strip_frontmatter(raw),
        )
    };
    let rendered_name = match host {
        EvaluatorClient::ClaudeCode | EvaluatorClient::ClaudeDesktop => key.replace('_', "-"),
        EvaluatorClient::OpenCode => kebab_dir(key),
        EvaluatorClient::Codex | EvaluatorClient::Hermes => name,
    };
    let rendered_name = serde_json::to_string(&rendered_name).map_err(integrity)?;
    let description = serde_json::to_string(&description).map_err(integrity)?;
    let instructions = match host {
        EvaluatorClient::ClaudeCode | EvaluatorClient::ClaudeDesktop => instructions.trim(),
        _ => instructions.trim_end(),
    };
    Ok(format!(
        "---\nname: {rendered_name}\ndescription: {description}\n---\n\n{instructions}\n"
    ))
}

struct PlanIdentity {
    publication_id: PublicationId,
    resource_id: ManagedResourceId,
    generation: i64,
    host: EvaluatorClient,
}

fn build_plan(
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
            manifest.files.iter().map(move |(path, file)| FileReadback {
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

fn kebab_dir(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    let mut last_dash = true;
    for c in id.chars() {
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
