//! `skills list` subcommand.
//!
//! Scans the services tree's skills directory for skill configs, rendering
//! either a filtered summary table or, when a skill name is given, a
//! single-skill detail card with an instructions preview. A managed skill of
//! the organization's inventory, read through the owner-bound
//! `Arc<OrganizationSkillResolver>` the `managed_resources` member provides,
//! replaces the same-keyed disk skill or, when withheld, suppresses it. When
//! the resolver cannot be resolved (no reachable database), only disk skills
//! are listed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_extension::cli::{CommandHost, CommandOutput};
use systemprompt_identifiers::SkillId;
use systemprompt_manifest::SKILL_CONFIG_FILENAME;
use systemprompt_marketplace::ManagedSkillResolution;
use systemprompt_marketplace::managed::{ManagedSkill, OrganizationSkillResolver};
use systemprompt_models::path_confine::confine_within;
use systemprompt_models::text::truncate_with_ellipsis;

use super::types::{SkillDetail, SkillListOutput, SkillSummary, parse_skill_from_config};

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(
        help = "Skill ID to show details (optional)",
        value_parser = systemprompt_extension::cli::parsers::parse_skill_id
    )]
    pub name: Option<SkillId>,

    #[arg(long, help = "Show only enabled skills")]
    pub enabled: bool,

    #[arg(long, help = "Show only disabled skills", conflicts_with = "enabled")]
    pub disabled: bool,
}

pub(super) async fn execute(args: ListArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let skills_path = get_skills_path(host)?;
    if let Some(name) = args.name {
        return show_resolved_skill(&name, host).await;
    }
    let mut skills = scan_skills(&skills_path)?;
    let Some(resolver) = organization_skills(host).await else {
        return Ok(render_list(args.enabled, args.disabled, skills));
    };
    for owned in resolver.owned_skills().await? {
        skills.retain(|item| item.skill_id.as_str() != owned.key);
        match owned.resolution {
            ManagedSkillResolution::Published(managed) => skills.push(SkillSummary {
                file_path: Some(managed_path(&managed)),
                skill_id: managed.id,
                name: managed.name.clone(),
                display_name: managed.name,
                enabled: true,
                tags: Vec::new(),
            }),
            ManagedSkillResolution::Withheld(_) => {},
            ManagedSkillResolution::NotManaged => {
                return Err(anyhow!("Managed skill '{}' lost its binding", owned.key));
            },
        }
    }
    skills.sort_by(|left, right| left.skill_id.cmp(&right.skill_id));
    Ok(render_list(args.enabled, args.disabled, skills))
}

fn managed_path(skill: &ManagedSkill) -> String {
    format!(
        "managed://{}@{}",
        skill.id.as_str(),
        skill.bundle_digest.as_str()
    )
}

pub fn execute_with_path(args: ListArgs, skills_path: &Path) -> Result<CommandOutput> {
    if let Some(name) = args.name {
        return show_skill_detail(&name, skills_path);
    }

    let skills = scan_skills(skills_path)?;

    Ok(render_list(args.enabled, args.disabled, skills))
}

fn render_list(enabled: bool, disabled: bool, skills: Vec<SkillSummary>) -> CommandOutput {
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

    CommandOutput::table_of(
        vec!["skill_id", "name", "enabled", "tags", "file_path"],
        &output.skills,
    )
    .with_title("Skills")
}

async fn organization_skills(host: &dyn CommandHost) -> Option<Arc<OrganizationSkillResolver>> {
    let resolved = match host.resolved().await {
        Ok(resolved) => resolved,
        Err(error) => {
            tracing::warn!(
                %error,
                "managed skills are not resolved: no command kernel; listing disk skills only"
            );
            return None;
        },
    };
    resolved
        .get::<Arc<OrganizationSkillResolver>>()
        .inspect_err(|error| {
            tracing::warn!(
                %error,
                "managed skills are not resolved: no organization skill resolver; listing disk skills only"
            );
        })
        .ok()
}

pub async fn show_resolved_skill(
    skill_id: &SkillId,
    host: &dyn CommandHost,
) -> Result<CommandOutput> {
    if let Some(resolver) = organization_skills(host).await {
        match resolver.resolve_owned(skill_id.as_str()).await? {
            ManagedSkillResolution::Withheld(reason) => {
                return Err(anyhow!(
                    "Skill '{}' is managed but withheld ({})",
                    skill_id,
                    reason.as_str()
                ));
            },
            ManagedSkillResolution::NotManaged => {},
            ManagedSkillResolution::Published(skill) => {
                let output = SkillDetail {
                    file_path: Some(managed_path(&skill)),
                    skill_id: skill.id,
                    name: skill.name.clone(),
                    display_name: skill.name,
                    description: skill.description,
                    enabled: true,
                    tags: Vec::new(),
                    category: Some(format!("managed generation {}", skill.generation)),
                    instructions_preview: truncate_with_ellipsis(&skill.instructions, 200),
                };
                return Ok(CommandOutput::card_value(
                    format!("Skill: {skill_id}"),
                    &output,
                ));
            },
        }
    }
    show_skill_detail(skill_id, &get_skills_path(host)?)
}

fn get_skills_path(host: &dyn CommandHost) -> Result<PathBuf> {
    Ok(host.services_tree()?.skills())
}

pub fn show_skill_detail(skill_id: &SkillId, skills_path: &Path) -> Result<CommandOutput> {
    let skill_dir = skills_path.join(skill_id.as_str());

    if !skill_dir.exists() {
        return Err(anyhow!("Skill '{}' not found", skill_id));
    }
    let skill_dir = confine_within(skills_path, &skill_dir)?;

    let config_path = skill_dir.join(SKILL_CONFIG_FILENAME);

    if !config_path.exists() {
        return Err(anyhow!(
            "Skill '{}' has no {} file",
            skill_id,
            SKILL_CONFIG_FILENAME
        ));
    }
    let config_path = confine_within(&skill_dir, &config_path)?;

    let parsed = parse_skill_from_config(&config_path, &skill_dir)?;

    let instructions_preview = truncate_with_ellipsis(&parsed.instructions, 200);

    let output = SkillDetail {
        skill_id: skill_id.clone(),
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
        format!("Skill: {}", skill_id),
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
