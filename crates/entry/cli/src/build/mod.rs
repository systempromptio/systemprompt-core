use anyhow::{anyhow, bail, Context, Result};
use clap::Subcommand;
use std::path::Path;
use std::process::Command;

use systemprompt_core_logging::CliService;
use systemprompt_loader::ExtensionLoader;
use systemprompt_models::BuildType;

use crate::common::project::ProjectRoot;

#[derive(Subcommand, Clone, Copy)]
pub enum BuildCommands {
    Mcp {
        #[arg(long, default_value = "false")]
        release: bool,
    },
}

pub fn execute(cmd: BuildCommands) -> Result<()> {
    match cmd {
        BuildCommands::Mcp { release } => build_mcp(release),
    }
}

fn build_mcp(release: bool) -> Result<()> {
    let project_root = ProjectRoot::discover().map_err(|e| anyhow!("{}", e))?;
    let root = project_root.as_path();

    let extensions = ExtensionLoader::get_enabled_mcp_extensions(root);

    if extensions.is_empty() {
        CliService::info("No MCP extensions found");
        return Ok(());
    }

    CliService::section("Building MCP Extensions");

    for ext in &extensions {
        let binary = ext.binary_name().ok_or_else(|| {
            anyhow!(
                "Extension {} has no binary defined",
                ext.manifest.extension.name
            )
        })?;

        let build_type = ext.build_type();

        match build_type {
            BuildType::Workspace => {
                build_workspace_crate(root, binary, release)?;
            },
            BuildType::Submodule => {
                build_submodule_crate(&ext.path, root, release)?;
            },
        }
    }

    CliService::success("All MCP extensions built");
    Ok(())
}

fn build_workspace_crate(project_root: &Path, package: &str, release: bool) -> Result<()> {
    CliService::info(&format!("Building {} (workspace)", package));

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

    CliService::success(&format!("  ✓ {}", package));
    Ok(())
}

fn build_submodule_crate(extension_path: &Path, project_root: &Path, release: bool) -> Result<()> {
    let name = extension_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("Invalid extension path: {}", extension_path.display()))?;

    CliService::info(&format!("Building {} (submodule)", name));

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

    CliService::success(&format!("  ✓ {}", name));
    Ok(())
}

fn find_cargo_manifest(project_root: &Path) -> Result<std::path::PathBuf> {
    let manifest = project_root.join("Cargo.toml");
    if manifest.exists() {
        return Ok(manifest);
    }
    bail!("Cargo.toml not found in project root")
}
