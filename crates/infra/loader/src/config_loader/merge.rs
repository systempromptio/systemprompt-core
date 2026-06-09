use std::collections::HashMap;
use std::fs;
use std::hash::Hash;
use std::path::Path;

use systemprompt_models::services::{IncludableString, ServicesConfig, SkillsConfig};

use crate::error::{ConfigLoadError, ConfigLoadResult};

/// Duplicate keys across includes (or include-vs-root) are a hard error: there
/// is no "last writer wins", so two includes silently shadowing each other is
/// impossible. AI providers are accumulated; `web` and `scheduler` carry
/// whichever side defined them (root has priority).
pub(super) fn merge_into(
    target: &mut ServicesConfig,
    include: ServicesConfig,
) -> ConfigLoadResult<()> {
    merge_no_dup(&mut target.agents, include.agents, |k| {
        ConfigLoadError::DuplicateAgent(k)
    })?;
    merge_no_dup(&mut target.mcp_servers, include.mcp_servers, |k| {
        ConfigLoadError::DuplicateMcpServer(k)
    })?;
    merge_no_dup(&mut target.plugins, include.plugins, |k| {
        ConfigLoadError::DuplicatePlugin(k)
    })?;
    merge_no_dup(&mut target.marketplaces, include.marketplaces, |k| {
        ConfigLoadError::DuplicateMarketplace(k.as_str().to_owned())
    })?;
    merge_no_dup(&mut target.external_agents, include.external_agents, |k| {
        ConfigLoadError::DuplicateExternalAgent(k.as_str().to_owned())
    })?;

    if include.scheduler.is_some() && target.scheduler.is_none() {
        target.scheduler = include.scheduler;
    }

    if !include.ai.providers.is_empty() {
        if target.ai.providers.is_empty() {
            target.ai = include.ai;
        } else {
            for (name, provider) in include.ai.providers {
                target.ai.providers.insert(name, provider);
            }
        }
    }

    if include.web.is_some() {
        target.web = include.web;
    }

    merge_skills(&mut target.skills, include.skills)?;

    Ok(())
}

fn merge_no_dup<K, V, E>(
    target: &mut HashMap<K, V>,
    other: HashMap<K, V>,
    on_dup: impl Fn(K) -> E,
) -> Result<(), E>
where
    K: Eq + Hash,
{
    for (key, value) in other {
        if target.contains_key(&key) {
            return Err(on_dup(key));
        }
        target.insert(key, value);
    }
    Ok(())
}

fn merge_skills(target: &mut SkillsConfig, partial: SkillsConfig) -> ConfigLoadResult<()> {
    if partial.auto_discover {
        target.auto_discover = true;
    }
    if partial.skills_path.is_some() {
        target.skills_path = partial.skills_path;
    }
    for (id, skill) in partial.skills {
        if target.skills.contains_key(&id) {
            return Err(ConfigLoadError::DuplicateSkill(id));
        }
        target.skills.insert(id, skill);
    }
    Ok(())
}

pub(super) fn warn_on_authored_card_skills(config: &ServicesConfig) {
    for (name, agent) in &config.agents {
        if !agent.card.skills.is_empty() {
            tracing::warn!(
                agent = %name,
                count = agent.card.skills.len(),
                "deprecated: agent YAML authors card.skills; this field is ignored. \
                 A2A card.skills is now derived from metadata.skills against the \
                 services/skills/ catalog. Remove the card.skills: array from this \
                 agent's config.yaml."
            );
        }
    }
}

pub(super) fn resolve_system_prompt_includes(
    base_path: &Path,
    config: &mut ServicesConfig,
) -> ConfigLoadResult<()> {
    for (name, agent) in &mut config.agents {
        if let Some(ref system_prompt) = agent.metadata.system_prompt
            && let Some(include_path) = system_prompt.strip_prefix("!include ")
        {
            let full_path = base_path.join(include_path.trim());
            let resolved = fs::read_to_string(&full_path).map_err(|e| ConfigLoadError::Io {
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
            let resolved = fs::read_to_string(&full_path).map_err(|e| ConfigLoadError::Io {
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
