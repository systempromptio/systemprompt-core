use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use std::path::Path;
use std::process::Command;

use systemprompt_core_logging::CliService;
use systemprompt_loader::ExtensionLoader;
use systemprompt_models::BuildType;

use super::types::{BuildExtensionRow, BuildOutput};
use crate::shared::command_result::CommandResult;
use crate::shared::project::ProjectRoot;
use crate::CliConfig;

#[derive(Debug, Clone, Copy, Args)]
pub struct McpArgs {
    #[arg(long, default_value = "false", help = "Build in release mode")]
    pub release: bool,
}

pub fn execute(args: McpArgs, config: &CliConfig) -> Result<CommandResult<BuildOutput>> {
    let project_root = ProjectRoot::discover().map_err(|e| anyhow!("{}", e))?;
    let root = project_root.as_path();

    let extensions = ExtensionLoader::get_enabled_mcp_extensions(root);

    if extensions.is_empty() {
        let output = BuildOutput {
            extensions: vec![],
            total: 0,
            successful: 0,
            release_mode: args.release,
        };
        return Ok(CommandResult::list(output).with_title("Build MCP Extensions"));
    }

    if !config.is_json_output() {
        CliService::section("Building MCP Extensions");
    }

    let mut built_extensions = Vec::new();
    let mut successful = 0;

    for ext in &extensions {
        let binary = ext.binary_name().ok_or_else(|| {
            anyhow!(
                "Extension {} has no binary defined",
                ext.manifest.extension.name
            )
        })?;

        let build_type = ext.build_type();
        let build_type_str = match build_type {
            BuildType::Workspace => "workspace",
            BuildType::Submodule => "submodule",
        };

        let build_result = match build_type {
            BuildType::Workspace => build_workspace_crate(root, binary, args.release, config),
            BuildType::Submodule => build_submodule_crate(&ext.path, root, args.release, config),
        };

        let status = match build_result {
            Ok(()) => {
                successful += 1;
                "success".to_string()
            }
            Err(e) => format!("failed: {}", e),
        };

        built_extensions.push(BuildExtensionRow {
            name: ext.manifest.extension.name.clone(),
            build_type: build_type_str.to_string(),
            status,
        });
    }

    let total = built_extensions.len();
    let output = BuildOutput {
        extensions: built_extensions,
        total,
        successful,
        release_mode: args.release,
    };

    if !config.is_json_output() {
        if successful == total {
            CliService::success(&format!("All {} MCP extensions built", total));
        } else {
            CliService::warning(&format!(
                "Built {}/{} MCP extensions successfully",
                successful, total
            ));
        }
    }

    Ok(CommandResult::table(output).with_title("Build MCP Extensions"))
}

fn build_workspace_crate(
    project_root: &Path,
    package: &str,
    release: bool,
    config: &CliConfig,
) -> Result<()> {
    if !config.is_json_output() {
        CliService::info(&format!("Building {} (workspace)", package));
    }

    let mut args = vec!["build", "-p", package];
    if release {
        args.push("--release");
    }

    let cargo_manifest = find_cargo_manifest(project_root)?;

    let status = Command::new("cargo")
        .args(&args)
        .arg("--manifest-path")
        .arg(&cargo_manifest)
        .arg("--target-dir")
        .arg(project_root.join("target"))
        .status()
        .context("Failed to execute cargo")?;

    if !status.success() {
        bail!("Failed to build {}", package);
    }

    if !config.is_json_output() {
        CliService::success(&format!("  {} built", package));
    }
    Ok(())
}

fn build_submodule_crate(
    extension_path: &Path,
    project_root: &Path,
    release: bool,
    config: &CliConfig,
) -> Result<()> {
    let name = extension_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("Invalid extension path: {}", extension_path.display()))?;

    if !config.is_json_output() {
        CliService::info(&format!("Building {} (submodule)", name));
    }

    let mut args = vec!["build"];
    if release {
        args.push("--release");
    }

    let target_dir = project_root.join("target");

    let status = Command::new("cargo")
        .args(&args)
        .arg("--target-dir")
        .arg(&target_dir)
        .current_dir(extension_path)
        .status()
        .context("Failed to execute cargo")?;

    if !status.success() {
        bail!("Failed to build {} at {}", name, extension_path.display());
    }

    if !config.is_json_output() {
        CliService::success(&format!("  {} built", name));
    }
    Ok(())
}

fn find_cargo_manifest(project_root: &Path) -> Result<std::path::PathBuf> {
    let manifest = project_root.join("Cargo.toml");
    if manifest.exists() {
        return Ok(manifest);
    }
    bail!("Cargo.toml not found in project root")
}
