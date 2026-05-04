use std::fs;
use std::path::Path;

use systemprompt_models::services::{
    ContentConfig, IncludableString, PartialServicesConfig, ServicesConfig, SkillsConfig,
};

use crate::error::{ConfigLoadError, ConfigLoadResult};

pub(super) fn merge_partial(
    target: &mut ServicesConfig,
    partial: PartialServicesConfig,
) -> ConfigLoadResult<()> {
    for (name, agent) in partial.agents {
        if target.agents.contains_key(&name) {
            return Err(ConfigLoadError::DuplicateAgent(name));
        }
        target.agents.insert(name, agent);
    }

    for (name, mcp) in partial.mcp_servers {
        if target.mcp_servers.contains_key(&name) {
            return Err(ConfigLoadError::DuplicateMcpServer(name));
        }
        target.mcp_servers.insert(name, mcp);
    }

    if partial.scheduler.is_some() && target.scheduler.is_none() {
        target.scheduler = partial.scheduler;
    }

    if let Some(ai) = partial.ai {
        if target.ai.providers.is_empty() && !ai.providers.is_empty() {
            target.ai = ai;
        } else {
            for (name, provider) in ai.providers {
                target.ai.providers.insert(name, provider);
            }
        }
    }

    if partial.web.is_some() {
        target.web = partial.web;
    }

    for (name, plugin) in partial.plugins {
        if target.plugins.contains_key(&name) {
            return Err(ConfigLoadError::DuplicatePlugin(name));
        }
        target.plugins.insert(name, plugin);
    }

    merge_skills(target, partial.skills)?;
    merge_content(&mut target.content, partial.content)?;

    Ok(())
}

fn merge_skills(target: &mut ServicesConfig, partial: SkillsConfig) -> ConfigLoadResult<()> {
    if partial.auto_discover {
        target.skills.auto_discover = true;
    }
    if partial.skills_path.is_some() {
        target.skills.skills_path = partial.skills_path;
    }
    for (id, skill) in partial.skills {
        if target.skills.skills.contains_key(&id) {
            return Err(ConfigLoadError::DuplicateSkill(id));
        }
        target.skills.skills.insert(id, skill);
    }
    Ok(())
}

fn merge_content(target: &mut ContentConfig, partial: ContentConfig) -> ConfigLoadResult<()> {
    for (name, source) in partial.sources {
        if target.sources.contains_key(&name) {
            return Err(ConfigLoadError::DuplicateContentSource(name));
        }
        target.sources.insert(name, source);
    }

    for (name, source) in partial.raw.content_sources {
        if target.raw.content_sources.contains_key(&name) {
            return Err(ConfigLoadError::DuplicateContentSource(name));
        }
        target.raw.content_sources.insert(name, source);
    }

    for (name, category) in partial.raw.categories {
        target.raw.categories.entry(name).or_insert(category);
    }

    if !partial.raw.metadata.default_author.is_empty() {
        target.raw.metadata = partial.raw.metadata;
    }

    Ok(())
}

pub(super) fn resolve_partial_includes(
    partial: &mut PartialServicesConfig,
    base_dir: &Path,
) -> ConfigLoadResult<()> {
    for (name, agent) in &mut partial.agents {
        if let Some(ref system_prompt) = agent.metadata.system_prompt {
            if let Some(include_path) = system_prompt.strip_prefix("!include ") {
                let full_path = base_dir.join(include_path.trim());
                let resolved = read_include(&full_path).map_err(|e| ConfigLoadError::Io {
                    path: full_path.clone(),
                    source: e,
                })?;
                tracing::debug!(
                    agent = %name,
                    path = %full_path.display(),
                    "resolved system_prompt include"
                );
                agent.metadata.system_prompt = Some(resolved);
            }
        }
    }

    for (key, skill) in &mut partial.skills.skills {
        let Some(instructions) = skill.instructions.as_ref() else {
            continue;
        };
        if let IncludableString::Include { path } = instructions {
            let full_path = base_dir.join(path.trim());
            let resolved = read_include(&full_path).map_err(|e| ConfigLoadError::Io {
                path: full_path.clone(),
                source: e,
            })?;
            tracing::debug!(
                skill = %key,
                path = %full_path.display(),
                "resolved skill instructions include"
            );
            skill.instructions = Some(IncludableString::Inline(resolved));
        }
    }

    Ok(())
}

pub(super) fn resolve_system_prompt_includes(
    base_path: &Path,
    config: &mut ServicesConfig,
) -> ConfigLoadResult<()> {
    for (name, agent) in &mut config.agents {
        if let Some(ref system_prompt) = agent.metadata.system_prompt {
            if let Some(include_path) = system_prompt.strip_prefix("!include ") {
                let full_path = base_path.join(include_path.trim());
                let resolved = read_include(&full_path).map_err(|e| ConfigLoadError::Io {
                    path: full_path.clone(),
                    source: e,
                })?;
                tracing::debug!(
                    agent = %name,
                    path = %full_path.display(),
                    "resolved system_prompt include"
                );
                agent.metadata.system_prompt = Some(resolved);
            }
        }
    }

    Ok(())
}

pub(super) fn resolve_skill_instruction_includes(
    base_path: &Path,
    config: &mut ServicesConfig,
) -> ConfigLoadResult<()> {
    for (key, skill) in &mut config.skills.skills {
        let Some(instructions) = skill.instructions.as_ref() else {
            continue;
        };
        if let IncludableString::Include { path } = instructions {
            let full_path = base_path.join(path.trim());
            let resolved = read_include(&full_path).map_err(|e| ConfigLoadError::Io {
                path: full_path.clone(),
                source: e,
            })?;
            tracing::debug!(
                skill = %key,
                path = %full_path.display(),
                "resolved skill instructions include"
            );
            skill.instructions = Some(IncludableString::Inline(resolved));
        }
    }
    Ok(())
}

fn read_include(path: &Path) -> std::io::Result<String> {
    fs::read_to_string(path)
}
