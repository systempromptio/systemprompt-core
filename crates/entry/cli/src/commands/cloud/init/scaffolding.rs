//! Writes the default `services/` directory tree for a new project.
//!
//! Creates the config, agent, MCP, content, web, and scheduler files from the
//! [`super::templates`] strings and clones the bundled systemprompt-admin MCP
//! server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use systemprompt_logging::CliService;

use super::templates::{
    admin_agent_config, admin_mcp_config, agent_config, ai_config, blog_list_template,
    blog_post_template, content_config, cookie_policy, page_list_template, page_template,
    privacy_policy, root_config, scheduler_config, web_config, web_metadata, welcome_blog_post,
};

const ADMIN_MCP_REPO: &str = "https://github.com/systempromptio/systemprompt-mcp-admin.git";

pub(super) fn generate_services_boilerplate(project_root: &Path, project_name: &str) -> Result<()> {
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
    write_file(
        &services_dir.join("ai/config.yaml"),
        &ai_config("anthropic"),
    )?;
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
    if git_dir.exists()
        && let Err(e) = std::fs::remove_dir_all(&git_dir)
    {
        tracing::warn!(path = %git_dir.display(), error = %e, "failed to remove .git dir");
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
