//! `cloud profile show`: render a profile's effective configuration.
//!
//! Loads the profile and services config, assembles a [`FullConfig`] scoped to
//! the requested [`ShowFilter`], and emits it as text, JSON, or YAML.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use std::collections::HashMap;
use systemprompt_config::ProfileBootstrap;
use systemprompt_loader::ConfigLoader;
use systemprompt_logging::CliService;
use systemprompt_models::{AiConfig, AppPaths, Config, ContentConfigRaw, SkillsConfig};

use super::ShowFilter;
use super::show_display::print_formatted_config;
use super::show_types::{FullConfig, SettingsOutput, build_env_config};
use crate::cli_settings::CliConfig;
use crate::context::CommandContext;
use crate::shared::{CommandOutput, render_result, resolve_profile_path};

pub(super) fn execute(
    name: Option<&str>,
    filter: ShowFilter,
    json_output: bool,
    yaml_output: bool,
    ctx: &CommandContext,
) -> Result<()> {
    let profile_path = resolve_profile_path(name, ctx.env.profile.as_deref(), None)?;

    CliService::section(&format!("Profile: {}", profile_path.display()));

    let config = Config::get().ok().or_else(|| {
        if initialize_config_from_profile(&profile_path).is_ok() {
            Config::get().ok()
        } else {
            None
        }
    });

    let services_config = ConfigLoader::load().ok();

    let paths = current_app_paths();
    let full_config =
        build_config_for_filter(filter, config, services_config.as_ref(), paths.as_ref());

    output_config(&full_config, json_output, yaml_output, &ctx.cli);
    Ok(())
}

fn initialize_config_from_profile(profile_path: &std::path::Path) -> Result<()> {
    use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};

    ProfileBootstrap::init_from_path(profile_path)?;
    SecretsBootstrap::init()?;
    systemprompt_config::try_init_config()?;
    Ok(())
}

fn current_app_paths() -> Option<AppPaths> {
    ProfileBootstrap::get()
        .ok()
        .and_then(|p| AppPaths::from_profile(&p.paths).ok())
}

fn build_config_for_filter(
    filter: ShowFilter,
    config: Option<&Config>,
    services_config: Option<&systemprompt_models::ServicesConfig>,
    paths: Option<&AppPaths>,
) -> FullConfig {
    match filter {
        ShowFilter::All => build_full_config(config, services_config, paths),
        ShowFilter::Agents => FullConfig::empty()
            .with_agents(services_config.map_or_else(HashMap::new, |s| s.agents.clone())),
        ShowFilter::Mcp => FullConfig::empty()
            .with_mcp_servers(services_config.map_or_else(HashMap::new, |s| s.mcp_servers.clone())),
        ShowFilter::Skills => {
            let mut full = FullConfig::empty();
            if let (Some(_cfg), Some(p)) = (config, paths)
                && let Some(skills) = load_skills_config(p)
            {
                full = full.with_skills(skills);
            }
            full
        },
        ShowFilter::Ai => FullConfig::empty()
            .with_ai(services_config.map_or_else(AiConfig::default, |s| s.ai.clone())),
        ShowFilter::Web => {
            FullConfig::empty().with_web(services_config.and_then(|s| s.web.clone()))
        },
        ShowFilter::Content => {
            let mut full = FullConfig::empty();
            if let Some(p) = paths
                && let Some(content) = load_content_config(p)
            {
                full = full.with_content(content);
            }
            full
        },
        ShowFilter::Env => config.map_or_else(FullConfig::empty, |cfg| {
            FullConfig::empty().with_environment(build_env_config(cfg, paths))
        }),
        ShowFilter::Settings => {
            let mut full = FullConfig::empty();
            if let Some(settings) = services_config.map(build_settings_output) {
                full = full.with_settings(settings);
            }
            full
        },
    }
}

fn build_full_config(
    config: Option<&Config>,
    services_config: Option<&systemprompt_models::ServicesConfig>,
    paths: Option<&AppPaths>,
) -> FullConfig {
    let mut full = FullConfig::empty();

    if let Some(cfg) = config {
        full = full.with_environment(build_env_config(cfg, paths));
        if let Some(p) = paths
            && let Some(skills) = load_skills_config(p)
        {
            full = full.with_skills(skills);
        }
    }

    if let Some(sc) = services_config {
        full = full
            .with_settings(build_settings_output(sc))
            .with_agents(sc.agents.clone())
            .with_mcp_servers(sc.mcp_servers.clone())
            .with_ai(sc.ai.clone())
            .with_web(sc.web.clone());
    }

    if let Some(p) = paths
        && let Some(content) = load_content_config(p)
    {
        full = full.with_content(content);
    }

    full
}

fn build_settings_output(services_config: &systemprompt_models::ServicesConfig) -> SettingsOutput {
    SettingsOutput {
        agent_port_range: services_config.settings.agent_port_range,
        mcp_port_range: services_config.settings.mcp_port_range,
        auto_start_enabled: services_config.settings.auto_start_enabled,
        validation_strict: services_config.settings.validation_strict,
        schema_validation_mode: services_config.settings.schema_validation_mode.clone(),
    }
}

fn load_skills_config(paths: &AppPaths) -> Option<SkillsConfig> {
    let skills_path = paths.system().skills().to_path_buf();
    if !skills_path.exists() {
        return None;
    }
    let config_file = skills_path.join("skills.yaml");
    if !config_file.exists() {
        return None;
    }

    let content = match std::fs::read_to_string(&config_file) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(path = %config_file.display(), error = %e, "Failed to read skills config");
            return None;
        },
    };

    match serde_yaml::from_str(&content) {
        Ok(config) => Some(config),
        Err(e) => {
            tracing::warn!(path = %config_file.display(), error = %e, "Failed to parse skills config");
            None
        },
    }
}

pub(super) fn load_content_config(paths: &AppPaths) -> Option<ContentConfigRaw> {
    let path = paths.system().content_config().to_path_buf();
    if !path.exists() {
        return None;
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "Failed to read content config");
            return None;
        },
    };

    match serde_yaml::from_str(&content) {
        Ok(config) => Some(config),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "Failed to parse content config");
            None
        },
    }
}

fn output_config(config: &FullConfig, json_output: bool, yaml_output: bool, cli: &CliConfig) {
    if json_output {
        let result = CommandOutput::card_value("Profile Configuration", config);
        render_result(&result, cli);
    } else if yaml_output {
        CliService::yaml(config);
    } else {
        print_formatted_config(config);
    }
}
