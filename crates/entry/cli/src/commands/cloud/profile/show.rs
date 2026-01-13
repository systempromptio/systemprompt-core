use anyhow::{bail, Result};
use std::path::PathBuf;
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_core_logging::CliService;
use systemprompt_loader::EnhancedConfigLoader;
use systemprompt_models::{AppPaths, Config, ContentConfigRaw, SkillsConfig};

use super::show_display::print_formatted_config;
use super::show_types::{
    CoreEnvVars, DatabaseEnvVars, EnvironmentConfig, FullConfig, JwtEnvVars, PathsEnvVars,
    RateLimitEnvVars, SettingsOutput, SystemPromptEnvVars,
};
use super::ShowFilter;
use crate::cli_settings::CliConfig;

pub fn execute(
    name: Option<&str>,
    filter: ShowFilter,
    json_output: bool,
    yaml_output: bool,
    config: &CliConfig,
) -> Result<()> {
    let _ = config; // Output format controlled by json_output/yaml_output flags
    let profile_path = resolve_profile_path(name)?;

    CliService::section(&format!("Profile: {}", profile_path.display()));

    let config = Config::get()?;
    let loader = EnhancedConfigLoader::from_env()?;
    let services_config = loader.load().ok();

    let full_config = build_config_for_filter(filter, config, services_config.as_ref());

    output_config(&full_config, json_output, yaml_output);
    Ok(())
}

fn resolve_profile_path(name: Option<&str>) -> Result<PathBuf> {
    if let Some(profile_name) = name {
        let ctx = ProjectContext::discover();
        let profile_path = ctx.profile_path(profile_name, ProfilePath::Config);

        if !profile_path.exists() {
            bail!(
                "Profile '{}' not found at {}",
                profile_name,
                profile_path.display()
            );
        }

        return Ok(profile_path);
    }

    if let Ok(path) = std::env::var("SYSTEMPROMPT_PROFILE") {
        let profile_path = PathBuf::from(&path);
        if profile_path.exists() {
            return Ok(profile_path);
        }
        bail!("Profile from SYSTEMPROMPT_PROFILE not found: {}", path);
    }

    bail!(
        "No profile specified and SYSTEMPROMPT_PROFILE not set.\nUsage: systemprompt cloud \
         profile show <name>"
    );
}

fn build_config_for_filter(
    filter: ShowFilter,
    config: &Config,
    services_config: Option<&systemprompt_models::ServicesConfig>,
) -> FullConfig {
    match filter {
        ShowFilter::All => build_full_config(config, services_config),
        ShowFilter::Agents => FullConfig::empty().with_agents(
            services_config
                .map(|s| s.agents.clone())
                .unwrap_or_default(),
        ),
        ShowFilter::Mcp => FullConfig::empty().with_mcp_servers(
            services_config
                .map(|s| s.mcp_servers.clone())
                .unwrap_or_default(),
        ),
        ShowFilter::Skills => {
            let mut full = FullConfig::empty();
            if let Some(skills) = load_skills_config(config) {
                full = full.with_skills(skills);
            }
            full
        },
        ShowFilter::Ai => {
            FullConfig::empty().with_ai(services_config.map(|s| s.ai.clone()).unwrap_or_default())
        },
        ShowFilter::Web => {
            FullConfig::empty().with_web(services_config.map(|s| s.web.clone()).unwrap_or_default())
        },
        ShowFilter::Content => {
            let mut full = FullConfig::empty();
            if let Some(content) = load_content_config() {
                full = full.with_content(content);
            }
            full
        },
        ShowFilter::Env => FullConfig::empty().with_environment(build_env_config(config)),
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
    config: &Config,
    services_config: Option<&systemprompt_models::ServicesConfig>,
) -> FullConfig {
    let mut full = FullConfig::empty().with_environment(build_env_config(config));

    if let Some(sc) = services_config {
        full = full
            .with_settings(build_settings_output(sc))
            .with_agents(sc.agents.clone())
            .with_mcp_servers(sc.mcp_servers.clone())
            .with_ai(sc.ai.clone())
            .with_web(sc.web.clone());
    }

    if let Some(skills) = load_skills_config(config) {
        full = full.with_skills(skills);
    }
    if let Some(content) = load_content_config() {
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

fn build_env_config(config: &Config) -> EnvironmentConfig {
    let env = systemprompt_models::config::Environment::detect();
    let verbosity = systemprompt_models::config::VerbosityLevel::resolve();

    EnvironmentConfig {
        core: CoreEnvVars {
            sitename: config.sitename.clone(),
            host: config.host.clone(),
            port: config.port,
            api_server_url: config.api_server_url.clone(),
            api_external_url: config.api_external_url.clone(),
            use_https: config.use_https,
            github_link: config.github_link.clone(),
            github_token: config
                .github_token
                .clone()
                .map(|_| "[REDACTED]".to_string()),
            cors_allowed_origins: config.cors_allowed_origins.clone(),
        },
        systemprompt: SystemPromptEnvVars {
            env: format!("{:?}", env),
            verbosity: format!("{:?}", verbosity),
            services_path: AppPaths::get()
                .ok()
                .map(|p| p.system().services().display().to_string()),
            skills_path: AppPaths::get()
                .ok()
                .map(|p| p.system().skills().display().to_string()),
            config_path: AppPaths::get()
                .ok()
                .map(|p| p.system().settings().display().to_string()),
        },
        database: DatabaseEnvVars {
            database_type: config.database_type.clone(),
            database_url: redact_database_url(&config.database_url),
        },
        jwt: JwtEnvVars {
            issuer: config.jwt_issuer.clone(),
            secret: "[REDACTED]".to_string(),
            access_token_expiration: config.jwt_access_token_expiration,
            refresh_token_expiration: config.jwt_refresh_token_expiration,
        },
        rate_limits: RateLimitEnvVars {
            disabled: config.rate_limits.disabled,
            burst_multiplier: config.rate_limits.burst_multiplier,
        },
        paths: PathsEnvVars {
            system_path: AppPaths::get()
                .map(|p| p.system().root().display().to_string())
                .unwrap_or_default(),
            services: AppPaths::get()
                .map(|p| p.system().services().display().to_string())
                .unwrap_or_default(),
            skills: AppPaths::get()
                .map(|p| p.system().skills().display().to_string())
                .unwrap_or_default(),
            services_config: AppPaths::get()
                .map(|p| p.system().settings().display().to_string())
                .unwrap_or_default(),
        },
    }
}

fn redact_database_url(url: &str) -> String {
    let Some(at_pos) = url.find('@') else {
        return url.to_string();
    };
    let Some(proto_end) = url.find("://") else {
        return url.to_string();
    };
    let protocol = &url[..proto_end + 3];
    let after_at = &url[at_pos..];
    format!("{}[REDACTED]{}", protocol, after_at)
}

fn load_skills_config(_config: &Config) -> Option<SkillsConfig> {
    let skills_path = AppPaths::get().ok()?.system().skills().to_path_buf();
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

fn load_content_config() -> Option<ContentConfigRaw> {
    let path = AppPaths::get()
        .ok()?
        .system()
        .content_config()
        .to_path_buf();
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

fn output_config(config: &FullConfig, json_output: bool, yaml_output: bool) {
    if json_output {
        CliService::json(config);
    } else if yaml_output {
        CliService::yaml(config);
    } else {
        print_formatted_config(config);
    }
}
