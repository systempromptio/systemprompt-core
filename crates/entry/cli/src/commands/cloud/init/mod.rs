//! `cloud init` project scaffolding.
//!
//! Creates the `.systemprompt/` directory with its ignore files, Dockerfile,
//! and entrypoint, and generates the default `services/` boilerplate for a new
//! project.

use anyhow::{Context, Result};
use std::path::Path;
use systemprompt_cloud::ProjectContext;
use systemprompt_logging::CliService;
use systemprompt_models::CliPaths;

mod scaffolding;
pub(super) mod templates;

use crate::cli_settings::CliConfig;
use scaffolding::generate_services_boilerplate;

const GITIGNORE_CONTENT: &str = "# Ignore sensitive files
credentials.json
tenants.json
**/secrets.json
docker/
storage/
";

const DOCKERIGNORE_CONTENT: &str = ".git
.gitignore
.gitmodules
target/debug
.cargo
.systemprompt/credentials.json
.systemprompt/tenants.json
.systemprompt/**/secrets.json
.systemprompt/docker
.systemprompt/storage
.env*
backup
docs
instructions
*.md
web/node_modules
.vscode
.idea
logs
*.log
";

fn entrypoint_content() -> String {
    format!(
        r#"#!/bin/sh
set -e

echo "Running database migrations..."
/app/bin/systemprompt {db_migrate_cmd}

echo "Starting services..."
exec /app/bin/systemprompt {services_serve_cmd} --foreground
"#,
        db_migrate_cmd = CliPaths::db_migrate_cmd(),
        services_serve_cmd = CliPaths::services_serve_cmd(),
    )
}

pub(super) fn execute(force: bool, _config: &CliConfig) -> Result<()> {
    let project_root = std::env::current_dir().context("Failed to get current directory")?;
    let ctx = ProjectContext::new(project_root.clone());
    let systemprompt_dir = ctx.systemprompt_dir();
    let services_dir = project_root.join("services");

    let project_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("systemprompt")
        .to_owned();

    CliService::section("Initialize Project");
    CliService::key_value("Project", &project_name);
    CliService::key_value("Root", &project_root.display().to_string());

    if systemprompt_dir.exists() {
        CliService::info(".systemprompt/ already exists");
    } else {
        create_systemprompt_dir(&systemprompt_dir, &project_root)?;
    }

    if !services_dir.exists() || force {
        if force && services_dir.exists() {
            CliService::warning("Removing existing services directory...");
            std::fs::remove_dir_all(&services_dir)
                .context("Failed to remove services directory")?;
        }
        generate_services_boilerplate(&project_root, &project_name)?;
    } else {
        CliService::info("services/ already exists (use --force to regenerate)");
    }

    CliService::section("Next Steps");
    CliService::info("1. systemprompt cloud auth login     # Authenticate");
    CliService::info("2. systemprompt cloud tenant create  # Create a tenant");
    CliService::info("3. systemprompt cloud profile create local  # Create a profile");

    Ok(())
}

fn create_systemprompt_dir(dir: &Path, project_root: &Path) -> Result<()> {
    std::fs::create_dir_all(dir).context("Failed to create .systemprompt directory")?;

    std::fs::write(dir.join(".gitignore"), GITIGNORE_CONTENT)
        .context("Failed to create .gitignore")?;
    CliService::info("  Created .systemprompt/.gitignore");

    std::fs::write(dir.join(".dockerignore"), DOCKERIGNORE_CONTENT)
        .context("Failed to create .dockerignore")?;
    CliService::info("  Created .systemprompt/.dockerignore");

    let dockerfile_content = systemprompt_cloud::deploy::generate_dockerfile_content(project_root);
    std::fs::write(dir.join("Dockerfile"), dockerfile_content)
        .context("Failed to create Dockerfile")?;
    CliService::info("  Created .systemprompt/Dockerfile");

    std::fs::write(dir.join("entrypoint.sh"), entrypoint_content())
        .context("Failed to create entrypoint.sh")?;
    CliService::info("  Created .systemprompt/entrypoint.sh");

    CliService::success("Created .systemprompt/");
    Ok(())
}

pub(super) fn ensure_project_scaffolding(project_root: &Path) -> Result<()> {
    let services_dir = project_root.join("services");
    let web_dir = project_root.join("web");

    if services_dir.exists() && web_dir.exists() {
        return Ok(());
    }

    let project_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("systemprompt")
        .to_owned();

    if !services_dir.exists() {
        CliService::info("Scaffolding services/ directory...");
        generate_services_boilerplate(project_root, &project_name)?;
    }

    if !web_dir.exists() {
        std::fs::create_dir_all(&web_dir)
            .with_context(|| format!("Failed to create directory: {}", web_dir.display()))?;
        CliService::info("Created web/ directory");
    }

    Ok(())
}
