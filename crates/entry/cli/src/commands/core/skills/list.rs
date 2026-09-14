//! `skills list` subcommand.
//!
//! Scans the profile's skills directory for skill configs, rendering either a
//! filtered summary table or, when a skill name is given, a single-skill detail
//! card with an instructions preview.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::path::Path;
use systemprompt_identifiers::SkillId;
use systemprompt_loader::ServicesRootBootstrap;
use systemprompt_models::SKILL_CONFIG_FILENAME;

use crate::CommandContext;
use crate::shared::{CommandOutput, truncate_with_ellipsis};

use super::types::{SkillDetailOutput, SkillListOutput, SkillSummary, parse_skill_from_config};

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(help = "Skill ID to show details (optional)")]
    pub name: Option<String>,

    #[arg(long, help = "Show only enabled skills")]
    pub enabled: bool,

    #[arg(long, help = "Show only disabled skills", conflicts_with = "enabled")]
    pub disabled: bool,
}

pub(super) async fn execute(args: ListArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let skills_path = get_skills_path()?;
    if let Some(name) = args.name {
        return show_resolved_skill(&name, ctx).await;
    }
    let mut skills = scan_skills(&skills_path)?;
    let Some((repository, owner)) = managed_context(ctx).await? else {
        return render_list(args.enabled, args.disabled, skills);
    };
    let resolver = systemprompt_marketplace::ManagedResourceResolver::new(repository.clone());
    let mut offset = 0;
    loop {
        let page = repository.list_resources(&owner, offset).await?;
        let count = page.len();
        for resource in page.into_iter().filter(|resource| resource.kind == "skill") {
            let managed = resolver
                .resolve_skill(&owner, &resource.resource_key)
                .await?
                .ok_or_else(|| {
                    anyhow!("Managed skill '{}' lost its binding", resource.resource_key)
                })?;
            skills.retain(|item| item.skill_id.as_str() != resource.resource_key);
            skills.push(SkillSummary {
                skill_id: managed.id,
                name: managed.name.clone(),
                display_name: managed.name,
                enabled: true,
                tags: Vec::new(),
                file_path: Some(format!(
                    "managed:generation:{}:{}",
                    managed.generation,
                    managed.bundle_digest.as_str()
                )),
            });
        }
        if count < 51 {
            break;
        }
        offset += 51;
    }
    skills.sort_by(|left, right| left.skill_id.cmp(&right.skill_id));
    render_list(args.enabled, args.disabled, skills)
}

pub fn execute_with_path(args: ListArgs, skills_path: &Path) -> Result<CommandOutput> {
    if let Some(name) = args.name {
        return show_skill_detail(&name, skills_path);
    }

    let skills = scan_skills(skills_path)?;

    render_list(args.enabled, args.disabled, skills)
}

fn render_list(enabled: bool, disabled: bool, skills: Vec<SkillSummary>) -> Result<CommandOutput> {
    let filtered: Vec<SkillSummary> = skills
        .into_iter()
        .filter(|s| {
            if enabled {
                s.enabled
            } else if disabled {
                !s.enabled
            } else {
                true
            }
        })
        .collect();

    let output = SkillListOutput { skills: filtered };

    Ok(CommandOutput::table_of(
        vec!["skill_id", "name", "enabled", "tags", "file_path"],
        &output.skills,
    )
    .with_title("Skills"))
}

async fn managed_context(
    ctx: &CommandContext,
) -> Result<
    Option<(
        systemprompt_marketplace::ManagedRepository,
        systemprompt_identifiers::UserId,
    )>,
> {
    let app = ctx.app_context().await?;
    let pool = app.db_pool().pool_arc()?;
    if pool.is_closed() {
        return Ok(None);
    }
    Ok(Some((
        systemprompt_marketplace::ManagedRepository::new(pool.as_ref().clone()),
        app.system_admin().id().clone(),
    )))
}

pub async fn show_resolved_skill(skill_name: &str, ctx: &CommandContext) -> Result<CommandOutput> {
    if let Some((repository, owner)) = managed_context(ctx).await? {
        let resolver = systemprompt_marketplace::ManagedResourceResolver::new(repository);
        if let Some(skill) = resolver.resolve_skill(&owner, skill_name).await? {
            let output = SkillDetailOutput {
                skill_id: skill.id,
                name: skill.name.clone(),
                display_name: skill.name,
                description: skill.description,
                enabled: true,
                tags: Vec::new(),
                category: Some(format!("managed generation {}", skill.generation)),
                file_path: Some(format!("managed:{}", skill.bundle_digest.as_str())),
                instructions_preview: truncate_with_ellipsis(&skill.instructions, 200),
            };
            return Ok(CommandOutput::card_value(
                format!("Skill: {skill_name}"),
                &output,
            ));
        }
    }
    show_skill_detail(skill_name, &get_skills_path()?)
}

fn get_skills_path() -> Result<std::path::PathBuf> {
    let profile = systemprompt_config::ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(ServicesRootBootstrap::active_path_or(
        &profile.paths.services,
        "skills",
    ))
}

pub fn show_skill_detail(skill_name: &str, skills_path: &Path) -> Result<CommandOutput> {
    let skill_dir = skills_path.join(skill_name);

    if !skill_dir.exists() {
        return Err(anyhow!("Skill '{}' not found", skill_name));
    }

    let config_path = skill_dir.join(SKILL_CONFIG_FILENAME);

    if !config_path.exists() {
        return Err(anyhow!(
            "Skill '{}' has no {} file",
            skill_name,
            SKILL_CONFIG_FILENAME
        ));
    }

    let parsed = parse_skill_from_config(&config_path, &skill_dir)?;

    let instructions_preview = truncate_with_ellipsis(&parsed.instructions, 200);

    let output = SkillDetailOutput {
        skill_id: SkillId::new(skill_name),
        name: parsed.name.clone(),
        display_name: parsed.name,
        description: parsed.description,
        enabled: parsed.enabled,
        tags: parsed.tags,
        category: parsed.category,
        file_path: Some(config_path.to_string_lossy().to_string()),
        instructions_preview,
    };

    Ok(CommandOutput::card_value(
        format!("Skill: {}", skill_name),
        &output,
    ))
}

fn scan_skills(skills_path: &Path) -> Result<Vec<SkillSummary>> {
    if !skills_path.exists() {
        return Ok(Vec::new());
    }

    let mut skills = Vec::new();

    for entry in std::fs::read_dir(skills_path)? {
        let entry = entry?;
        let skill_path = entry.path();

        if !skill_path.is_dir() {
            continue;
        }

        let config_path = skill_path.join(SKILL_CONFIG_FILENAME);
        if !config_path.exists() {
            continue;
        }

        match parse_skill_from_config(&config_path, &skill_path) {
            Ok(parsed) => {
                let dir_name = skill_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow!("Invalid skill directory name"))?;

                skills.push(SkillSummary {
                    skill_id: SkillId::new(dir_name),
                    name: parsed.name.clone(),
                    display_name: parsed.name,
                    enabled: parsed.enabled,
                    tags: parsed.tags,
                    file_path: Some(config_path.to_string_lossy().to_string()),
                });
            },
            Err(e) => {
                tracing::warn!(
                    path = %skill_path.display(),
                    error = %e,
                    "Failed to parse skill"
                );
            },
        }
    }

    skills.sort_by(|a, b| a.skill_id.cmp(&b.skill_id));
    Ok(skills)
}
