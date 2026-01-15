pub mod types;

use crate::cli_settings::CliConfig;
use crate::shared::{render_result, CommandResult, RenderingHints};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::process::Stdio;
use systemprompt_loader::ExtensionLoader;
use systemprompt_models::{ProfileBootstrap, SecretsBootstrap};
use tokio::process::Command;
use types::{CliCommandInfo, CliExtensionInfo, ExtListOutput};

#[derive(Debug, Subcommand)]
pub enum ExtCommands {
    #[command(about = "List available CLI extensions")]
    List,

    #[command(about = "Run a CLI extension command", trailing_var_arg = true)]
    Run(RunArgs),
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(help = "Extension name (binary name or manifest name)")]
    pub extension: String,

    #[arg(help = "Arguments to pass to the extension", trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub async fn execute(cmd: ExtCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        ExtCommands::List => {
            let result = list_extensions()?;
            render_result(&result);
            Ok(())
        },
        ExtCommands::Run(args) => run_extension(args, config).await,
    }
}

fn list_extensions() -> Result<CommandResult<ExtListOutput>> {
    let project_root = std::env::current_dir().context("Failed to get current directory")?;
    let extensions = ExtensionLoader::get_enabled_cli_extensions(&project_root);

    let extension_infos: Vec<CliExtensionInfo> = extensions
        .iter()
        .filter_map(|ext| {
            let binary = ext.binary_name()?.to_string();
            Some(CliExtensionInfo {
                name: ext.manifest.extension.name.clone(),
                binary,
                description: ext.manifest.extension.description.clone(),
                commands: ext
                    .commands()
                    .iter()
                    .map(|cmd| CliCommandInfo {
                        name: cmd.name.clone(),
                        description: cmd.description.clone(),
                    })
                    .collect(),
                enabled: ext.is_enabled(),
            })
        })
        .collect();

    let total = extension_infos.len();
    let output = ExtListOutput {
        extensions: extension_infos,
        total,
    };

    Ok(CommandResult::table(output)
        .with_title("CLI Extensions")
        .with_hints(RenderingHints {
            columns: Some(vec![
                "name".to_string(),
                "binary".to_string(),
                "description".to_string(),
                "commands".to_string(),
            ]),
            ..Default::default()
        }))
}

async fn run_extension(args: RunArgs, config: &CliConfig) -> Result<()> {
    let project_root = std::env::current_dir().context("Failed to get current directory")?;

    let extension = ExtensionLoader::find_cli_extension(&project_root, &args.extension)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "CLI extension '{}' not found. Use 'ext list' to see available extensions",
                args.extension
            )
        })?;

    let binary_name = extension
        .binary_name()
        .ok_or_else(|| anyhow::anyhow!("Extension '{}' has no binary defined", args.extension))?;

    let binary_path =
        ExtensionLoader::get_cli_binary_path(&project_root, binary_name).ok_or_else(|| {
            anyhow::anyhow!(
                "Binary '{}' not found. Build with: cargo build --release --package {}",
                binary_name,
                binary_name
            )
        })?;

    let profile_path = ProfileBootstrap::get_path().context("Profile path required")?;

    let mut cmd = Command::new(&binary_path);
    cmd.args(&args.args)
        .env("SYSTEMPROMPT_PROFILE", profile_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if let Ok(jwt_secret) = SecretsBootstrap::jwt_secret() {
        cmd.env("JWT_SECRET", jwt_secret);
    }

    if let Ok(database_url) = std::env::var("DATABASE_URL") {
        cmd.env("DATABASE_URL", database_url);
    }

    if config.is_json_output() {
        cmd.arg("--json");
    }

    let status = cmd.status().await.with_context(|| {
        format!(
            "Failed to execute extension binary: {}",
            binary_path.display()
        )
    })?;

    if status.success() {
        Ok(())
    } else {
        let code = status.code().unwrap_or(1);
        anyhow::bail!("Extension exited with code {}", code)
    }
}
