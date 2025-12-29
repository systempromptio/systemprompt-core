use std::collections::HashMap;
use std::path::Path;
use std::process::Output;
use tokio::fs;
use tokio::process::Command;

use super::orchestrator::{BuildError, BuildMode, Result};

pub async fn generate_theme(web_dir: &Path) -> Result<()> {
    tracing::debug!("Generating theme CSS and TypeScript config");

    let script_path = web_dir.join("scripts/generate-theme.js");
    if !script_path.exists() {
        return Err(BuildError::ThemeGenerationFailed(format!(
            "Theme generation script not found at: {}",
            script_path.display()
        )));
    }

    let output = run_node_script(web_dir, "scripts/generate-theme.js").await?;
    check_command_success(&output, build_theme_error)?;
    log_stdout(&output, "Theme generation output");
    Ok(())
}

async fn run_node_script(web_dir: &Path, script: &str) -> Result<Output> {
    Command::new("node")
        .current_dir(web_dir)
        .arg(script)
        .output()
        .await
        .map_err(|e| BuildError::ProcessError(format!("Failed to execute {script}: {e}")))
}

fn build_theme_error(output: &Output) -> BuildError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    BuildError::ThemeGenerationFailed(format!("Theme generation script failed:\n{stderr}"))
}

pub async fn compile_typescript(web_dir: &Path) -> Result<()> {
    tracing::debug!("Compiling TypeScript");

    let output = run_npx_command(web_dir, &["tsc", "-b"]).await?;
    check_command_success(&output, build_typescript_error)?;
    log_stdout(&output, "TypeScript compilation output");
    Ok(())
}

async fn run_npx_command(web_dir: &Path, args: &[&str]) -> Result<Output> {
    Command::new("npx")
        .current_dir(web_dir)
        .args(args)
        .output()
        .await
        .map_err(|e| BuildError::ProcessError(format!("Failed to execute npx {}: {e}", args[0])))
}

fn build_typescript_error(output: &Output) -> BuildError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    BuildError::TypeScriptFailed(format!(
        "TypeScript compilation failed:\nstdout: {stdout}\nstderr: {stderr}"
    ))
}

fn check_command_success(output: &Output, build_error: fn(&Output) -> BuildError) -> Result<()> {
    if output.status.success() {
        Ok(())
    } else {
        Err(build_error(output))
    }
}

fn log_stdout(output: &Output, context: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        tracing::debug!(output = %stdout.trim(), "{context}");
    }
}

pub async fn build_vite(web_dir: &Path, mode: &BuildMode) -> Result<()> {
    let mode_str = mode.as_str();
    maybe_remove_env_local(web_dir, mode)?;

    let env_vars = load_vite_env_vars(web_dir, mode_str)?;
    tracing::debug!(mode = %mode_str, "Building with Vite");

    let output = execute_vite_command(web_dir, mode_str, &env_vars).await?;
    log_stdout(&output, "Vite build output");
    Ok(())
}

fn maybe_remove_env_local(web_dir: &Path, mode: &BuildMode) -> Result<()> {
    if !matches!(mode, BuildMode::Production | BuildMode::Docker) {
        return Ok(());
    }

    let env_local = web_dir.join(".env.local");
    if !env_local.exists() {
        return Ok(());
    }

    std::fs::remove_file(&env_local)
        .map_err(|e| BuildError::ProcessError(format!("Failed to remove .env.local: {e}")))?;
    tracing::debug!("Removed .env.local to prevent override");
    Ok(())
}

async fn execute_vite_command(
    web_dir: &Path,
    mode_str: &str,
    env_vars: &HashMap<String, String>,
) -> Result<Output> {
    let mut command = Command::new("npx");
    command
        .current_dir(web_dir)
        .args(["vite", "build", "--mode", mode_str])
        .envs(env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())));

    let output = command
        .output()
        .await
        .map_err(|e| BuildError::ProcessError(format!("Failed to execute vite: {e}")))?;

    check_command_success(&output, build_vite_error)?;
    Ok(output)
}

fn build_vite_error(output: &Output) -> BuildError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    BuildError::ViteFailed(format!(
        "Vite build failed:\nstdout: {stdout}\nstderr: {stderr}"
    ))
}

fn load_vite_env_vars(web_dir: &Path, mode_str: &str) -> Result<HashMap<String, String>> {
    let env_file = web_dir.join(format!(".env.{mode_str}"));
    if !env_file.exists() {
        return Ok(HashMap::new());
    }

    let content = std::fs::read_to_string(&env_file)
        .map_err(|e| BuildError::ProcessError(format!("Failed to read env file: {e}")))?;

    let env_vars: HashMap<String, String> = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .filter(|(key, _)| key.trim().starts_with("VITE_"))
        .map(|(key, value)| {
            (
                key.trim().to_string(),
                value.trim().trim_matches('"').to_string(),
            )
        })
        .collect();

    tracing::debug!(count = env_vars.len(), file = %env_file.display(), "Loaded VITE variables");
    Ok(env_vars)
}

const CSS_FILES: &[&str] = &["content.css", "syntax-highlight.css"];

pub async fn organize_css(web_dir: &Path) -> Result<()> {
    tracing::debug!("Organizing CSS files");

    let dist_dir = web_dir.join("dist");
    let css_dir = dist_dir.join("css");

    fs::create_dir_all(&css_dir).await.map_err(|e| {
        BuildError::CssOrganizationFailed(format!("Failed to create css directory: {e}"))
    })?;

    for file_name in CSS_FILES {
        copy_css_file(&dist_dir, &css_dir, file_name).await?;
    }
    Ok(())
}

async fn copy_css_file(dist_dir: &Path, css_dir: &Path, file_name: &str) -> Result<()> {
    let source = dist_dir.join(file_name);
    if !source.exists() {
        tracing::warn!(file = %file_name, "CSS file not found, skipping");
        return Ok(());
    }
    do_copy_css(&source, &css_dir.join(file_name), file_name).await
}

async fn do_copy_css(source: &Path, dest: &Path, file_name: &str) -> Result<()> {
    fs::copy(source, dest).await.map_err(|e| {
        BuildError::CssOrganizationFailed(format!("Failed to copy {file_name} to css/: {e}"))
    })?;
    tracing::debug!(file = %file_name, "Copied CSS file to css/");
    Ok(())
}
