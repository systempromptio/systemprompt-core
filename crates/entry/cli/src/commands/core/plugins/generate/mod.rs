mod agents;
mod hooks;
mod marketplace;
mod mcp;
mod skills;

use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::path::{Path, PathBuf};

use crate::CliConfig;
use crate::shared::CommandResult;
use systemprompt_models::PluginConfigFile;

use super::types::{PluginGenerateAllOutput, PluginGenerateOutput};

const DEFAULT_AGENT_TOOLS: &str = "Read, Grep, Glob, Bash, Write, Edit, WebFetch, WebSearch";

#[derive(Debug, Clone, Args)]
pub struct GenerateArgs {
    #[arg(long, help = "Plugin ID to generate (generates all if omitted)")]
    pub id: Option<String>,

    #[arg(long, help = "Output directory (defaults to plugin directory)")]
    pub output_dir: Option<String>,
}

struct PluginGenerateContext<'a> {
    plugins_path: &'a Path,
    skills_path: &'a Path,
    services_path: &'a Path,
    output_dir_override: Option<&'a str>,
}

pub fn execute(
    args: &GenerateArgs,
    _config: &CliConfig,
) -> Result<CommandResult<PluginGenerateAllOutput>> {
    let profile = systemprompt_models::ProfileBootstrap::get().context("Failed to get profile")?;
    let plugins_path = PathBuf::from(profile.paths.plugins());
    let skills_path = PathBuf::from(profile.paths.skills());
    let services_path = PathBuf::from(&profile.paths.services);

    let plugin_ids = match &args.id {
        Some(id) => {
            let plugin_dir = plugins_path.join(id);
            if !plugin_dir.exists() {
                return Err(anyhow!("Plugin '{}' not found", id));
            }
            vec![id.clone()]
        },
        None => collect_plugin_ids(&plugins_path)?,
    };

    let ctx = PluginGenerateContext {
        plugins_path: &plugins_path,
        skills_path: &skills_path,
        services_path: &services_path,
        output_dir_override: args.output_dir.as_deref(),
    };

    let mut results = Vec::new();

    for plugin_id in &plugin_ids {
        let result = generate_plugin(plugin_id, &ctx)?;
        results.push(result);
    }

    let plugins_output_path = services_path
        .join("..")
        .join("storage")
        .join("files")
        .join("plugins");
    marketplace::generate_marketplace_json(&plugins_path, &plugins_output_path)?;

    let install_hint = extract_install_command(profile);

    let output = PluginGenerateAllOutput {
        results,
        install_command: install_hint,
    };

    Ok(CommandResult::text(output).with_title("Plugin Generation Complete"))
}

fn collect_plugin_ids(plugins_path: &Path) -> Result<Vec<String>> {
    if !plugins_path.exists() {
        return Ok(Vec::new());
    }

    let mut ids = Vec::new();
    for entry in std::fs::read_dir(plugins_path)? {
        let entry = entry?;
        if entry.path().is_dir() && entry.path().join("config.yaml").exists() {
            if let Some(name) = entry.file_name().to_str() {
                ids.push(name.to_string());
            }
        }
    }
    ids.sort();
    Ok(ids)
}

fn generate_plugin(
    plugin_id: &str,
    ctx: &PluginGenerateContext<'_>,
) -> Result<PluginGenerateOutput> {
    let config_path = ctx.plugins_path.join(plugin_id).join("config.yaml");
    let content = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    let plugin_file: PluginConfigFile = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", config_path.display()))?;
    let plugin = &plugin_file.plugin;

    let output_dir = ctx.output_dir_override.map_or_else(
        || {
            ctx.services_path
                .join("..")
                .join("storage")
                .join("files")
                .join("plugins")
                .join(plugin_id)
        },
        PathBuf::from,
    );

    let mut files_generated = Vec::new();

    skills::generate_skills(plugin, ctx.skills_path, &output_dir, &mut files_generated)?;
    agents::generate_agents(plugin, ctx.services_path, &output_dir, &mut files_generated)?;
    mcp::generate_mcp_json(plugin, ctx.services_path, &output_dir, &mut files_generated)?;
    hooks::generate_hooks_json(&plugin.hooks, &output_dir, &mut files_generated)?;
    marketplace::copy_scripts(
        plugin,
        ctx.plugins_path,
        plugin_id,
        &output_dir,
        &mut files_generated,
    )?;
    marketplace::generate_plugin_json(plugin, &output_dir, &mut files_generated)?;

    Ok(PluginGenerateOutput {
        plugin_id: systemprompt_identifiers::PluginId::new(plugin_id),
        files_generated,
        marketplace_path: output_dir.to_string_lossy().to_string(),
    })
}

fn extract_install_command(profile: &systemprompt_models::Profile) -> Option<String> {
    let github_link = profile.site.github_link.as_deref()?;
    let repo_path = github_link
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .rsplit("github.com/")
        .next()?;

    if repo_path.contains('/') {
        Some(format!("/plugin marketplace add {}", repo_path))
    } else {
        None
    }
}
