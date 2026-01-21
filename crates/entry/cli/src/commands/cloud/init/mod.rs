use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use systemprompt_cloud::ProjectContext;
use systemprompt_logging::CliService;
use systemprompt_models::CliPaths;

mod templates;

use super::dockerfile;
use crate::cli_settings::CliConfig;
use templates::{
    admin_agent_config, admin_mcp_config, agent_config, ai_config, blog_list_template,
    blog_post_template, content_config, cookie_policy, page_list_template, page_template,
    privacy_policy, root_config, scheduler_config, web_config, web_metadata, welcome_blog_post,
};

const ADMIN_MCP_REPO: &str = "https://github.com/systempromptio/systemprompt-mcp-admin.git";

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

pub fn execute(force: bool, _config: &CliConfig) -> Result<()> {
    let project_root = std::env::current_dir().context("Failed to get current directory")?;
    let ctx = ProjectContext::new(project_root.clone());
    let systemprompt_dir = ctx.systemprompt_dir();
    let services_dir = project_root.join("services");

    let project_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("systemprompt")
        .to_string();

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

    let dockerfile_content = dockerfile::generate_dockerfile_content(project_root);
    std::fs::write(dir.join("Dockerfile"), dockerfile_content)
        .context("Failed to create Dockerfile")?;
    CliService::info("  Created .systemprompt/Dockerfile");

    std::fs::write(dir.join("entrypoint.sh"), entrypoint_content())
        .context("Failed to create entrypoint.sh")?;
    CliService::info("  Created .systemprompt/entrypoint.sh");

    CliService::success("Created .systemprompt/");
    Ok(())
}

fn generate_services_boilerplate(project_root: &Path, project_name: &str) -> Result<()> {
    CliService::section("Creating Services Boilerplate");

    let services_dir = project_root.join("services");
    let logs_dir = project_root.join("logs");

    create_directories(&services_dir, &logs_dir)?;
    write_config_files(&services_dir, project_name)?;
    write_template_files(&services_dir)?;
    write_content_files(&services_dir, project_name)?;

    write_file(&services_dir.join("skills/.gitkeep"), "")?;

    clone_admin_mcp_server(&services_dir)?;

    CliService::success("Services boilerplate created");
    Ok(())
}

fn create_directories(services_dir: &Path, logs_dir: &Path) -> Result<()> {
    create_dir(services_dir)?;
    create_dir(&services_dir.join("config"))?;
    create_dir(&services_dir.join("agents"))?;
    create_dir(&services_dir.join("mcp"))?;
    create_dir(&services_dir.join("ai"))?;
    create_dir(&services_dir.join("content"))?;
    create_dir(&services_dir.join("content/blog"))?;
    create_dir(&services_dir.join("content/blog/welcome"))?;
    create_dir(&services_dir.join("content/legal"))?;
    create_dir(&services_dir.join("skills"))?;
    create_dir(&services_dir.join("web"))?;
    create_dir(&services_dir.join("web/templates"))?;
    create_dir(&services_dir.join("web/assets"))?;
    create_dir(&services_dir.join("scheduler"))?;

    create_dir(logs_dir)?;
    write_file(
        &logs_dir.join(".gitignore"),
        "# Ignore all log files\n*.log\n*.log.*\n",
    )?;

    Ok(())
}

fn write_config_files(services_dir: &Path, project_name: &str) -> Result<()> {
    write_file(&services_dir.join("config/config.yaml"), &root_config())?;
    write_file(
        &services_dir.join("agents/assistant.yaml"),
        &agent_config(project_name),
    )?;
    write_file(
        &services_dir.join("agents/admin.yaml"),
        &admin_agent_config(),
    )?;
    write_file(
        &services_dir.join("mcp/systemprompt-admin.yaml"),
        &admin_mcp_config(),
    )?;
    write_file(&services_dir.join("ai/config.yaml"), &ai_config())?;
    write_file(&services_dir.join("content/config.yaml"), &content_config())?;
    write_file(
        &services_dir.join("web/config.yaml"),
        &web_config(project_name),
    )?;
    write_file(
        &services_dir.join("web/metadata.yaml"),
        &web_metadata(project_name),
    )?;
    write_file(
        &services_dir.join("scheduler/config.yaml"),
        &scheduler_config(),
    )?;

    Ok(())
}

fn write_template_files(services_dir: &Path) -> Result<()> {
    write_file(
        &services_dir.join("web/templates/page.html"),
        &page_template(),
    )?;
    write_file(
        &services_dir.join("web/templates/blog-post.html"),
        &blog_post_template(),
    )?;
    write_file(
        &services_dir.join("web/templates/blog-list.html"),
        &blog_list_template(),
    )?;
    write_file(
        &services_dir.join("web/templates/page-list.html"),
        &page_list_template(),
    )?;

    Ok(())
}

fn write_content_files(services_dir: &Path, project_name: &str) -> Result<()> {
    write_file(
        &services_dir.join("content/blog/welcome/index.md"),
        &welcome_blog_post(project_name),
    )?;
    write_file(
        &services_dir.join("content/legal/privacy-policy.md"),
        &privacy_policy(project_name),
    )?;
    write_file(
        &services_dir.join("content/legal/cookie-policy.md"),
        &cookie_policy(project_name),
    )?;

    Ok(())
}

fn clone_admin_mcp_server(services_dir: &Path) -> Result<()> {
    let mcp_dir = services_dir.join("mcp/systemprompt-admin");

    if mcp_dir.exists() {
        CliService::info("systemprompt-admin MCP server already exists");
        return Ok(());
    }

    let spinner = CliService::spinner("Cloning systemprompt-admin MCP server...");

    let output = Command::new("git")
        .args(["clone", "--depth", "1", ADMIN_MCP_REPO])
        .arg(&mcp_dir)
        .output()
        .context("Failed to execute git clone")?;

    spinner.finish_and_clear();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        CliService::warning(&format!("Could not clone systemprompt-admin: {}", stderr));
        return Ok(());
    }

    let git_dir = mcp_dir.join(".git");
    if git_dir.exists() {
        std::fs::remove_dir_all(&git_dir).ok();
    }

    CliService::success("Cloned systemprompt-admin MCP server");
    Ok(())
}

fn create_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("Failed to create directory: {}", path.display()))
}

fn write_file(path: &Path, content: &str) -> Result<()> {
    std::fs::write(path, content)
        .with_context(|| format!("Failed to write file: {}", path.display()))?;
    CliService::info(&format!("  Created {}", path.display()));
    Ok(())
}
