//! Generation of Claude Code plugin output from `config.yaml` definitions.
//!
//! `execute` takes one or all plugins of the loaded services configuration
//! (parsed by the loader from the plugins directory) and materialises skill,
//! agent, MCP, script, and marketplace artifacts into the plugins storage
//! directory. The per-component generators live in the `agents`, `mcp`,
//! `skills`, and `marketplace` submodules.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agents;
pub mod marketplace;
pub mod mcp;
pub mod skills;

use anyhow::{Context, Result};
use clap::Args;
use std::path::{Path, PathBuf};

use systemprompt_extension::cli::{CommandHost, CommandOutput};
use systemprompt_identifiers::PluginId;

use super::types::{PluginGenerateAllOutput, PluginGenerateOutput};

const DEFAULT_AGENT_TOOLS: &str = "Read, Grep, Glob, Bash, Write, Edit, WebFetch, WebSearch";

#[derive(Debug, Clone, Args)]
pub struct GenerateArgs {
    #[arg(
        long,
        help = "Plugin ID to generate (generates all if omitted)",
        value_parser = systemprompt_extension::cli::parsers::parse_plugin_id
    )]
    pub id: Option<PluginId>,

    #[arg(long, help = "Output directory (defaults to plugin directory)")]
    pub output_dir: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct PluginGenerateContext<'a> {
    pub plugins_path: &'a Path,
    pub skills_path: &'a Path,
    pub services_path: &'a Path,
    pub output_dir_override: Option<&'a str>,
}

pub(super) async fn execute(args: &GenerateArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let tree = host.services_tree()?;
    let services_path = tree.root().to_path_buf();
    let plugins_path = tree.plugins();
    let skills_path = tree.skills();

    let services = crate::cli::services_config(host)
        .await
        .context("Failed to load services config")?;
    let plugins = super::selected_plugins(&services, args.id.as_ref())?;

    let ctx = PluginGenerateContext {
        plugins_path: &plugins_path,
        skills_path: &skills_path,
        services_path: &services_path,
        output_dir_override: args.output_dir.as_deref(),
    };

    let mut results = Vec::new();

    for plugin in plugins {
        results.push(generate_plugin(plugin, &ctx)?);
    }

    let plugins_output_path = services_path
        .join("..")
        .join("storage")
        .join("files")
        .join("plugins");
    marketplace::generate_marketplace_json(&services, &plugins_output_path)?;

    let install_hint = extract_install_command(tree.site_github_link());

    let output = PluginGenerateAllOutput {
        results,
        install_command: install_hint,
    };

    Ok(CommandOutput::card_value(
        "Plugin Generation Complete",
        &output,
    ))
}

pub fn generate_plugin(
    plugin: &systemprompt_manifest::PluginConfig,
    ctx: &PluginGenerateContext<'_>,
) -> Result<PluginGenerateOutput> {
    let plugin_id = &plugin.id;

    let output_dir = ctx.output_dir_override.map_or_else(
        || {
            ctx.services_path
                .join("..")
                .join("storage")
                .join("files")
                .join("plugins")
                .join(plugin_id.as_str())
        },
        PathBuf::from,
    );

    let mut files_generated = Vec::new();

    skills::generate_skills(plugin, ctx.skills_path, &output_dir, &mut files_generated)?;
    agents::generate_agents(plugin, ctx.services_path, &output_dir, &mut files_generated)?;
    mcp::generate_mcp_json(plugin, ctx.services_path, &output_dir, &mut files_generated)?;
    marketplace::copy_scripts(
        plugin,
        ctx.plugins_path,
        plugin_id,
        &output_dir,
        &mut files_generated,
    )?;
    marketplace::generate_plugin_json(plugin, &output_dir, &mut files_generated)?;

    Ok(PluginGenerateOutput {
        plugin_id: plugin_id.clone(),
        files_generated,
        marketplace_path: output_dir.to_string_lossy().to_string(),
    })
}

pub fn extract_install_command(github_link: Option<&str>) -> Option<String> {
    let repo_path = github_link?
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
