use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use systemprompt_core_logging::CliService;
use systemprompt_models::{Config, SystemPaths};

pub async fn build_web_assets() -> Result<()> {
    let config = Config::get()?;
    let web_path = SystemPaths::web_path(config);

    if !web_path.exists() {
        return Err(anyhow::anyhow!(
            "Profile Error: Web path does not exist\n\n  Path: {}\n  Field: paths.web_path\n\n  \
             To fix: Ensure the path exists or update your profile",
            web_path.display()
        ));
    }

    CliService::section("Building Web Assets");

    sync_content_images(&web_path, "blog")?;

    CliService::info("Running npm run build...");

    let mut cmd = Command::new("npm");
    cmd.args(["run", "build"])
        .current_dir(&web_path)
        .env("SYSTEMPROMPT_WEB_CONFIG_PATH", &config.web_config_path)
        .env("SYSTEMPROMPT_WEB_METADATA_PATH", &config.web_metadata_path);

    let output = cmd
        .output()
        .context("Failed to run npm build - is npm installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        CliService::error(&format!("npm stderr: {}", stderr));
        CliService::error(&format!("npm stdout: {}", stdout));
        return Err(anyhow::anyhow!("Web build failed"));
    }

    CliService::success("Web assets built successfully");
    Ok(())
}

fn sync_content_images(web_path: &Path, source: &str) -> Result<()> {
    let src = Path::new("services/web/assets/images").join(source);
    let dest = web_path.join("public/images").join(source);

    if !src.exists() {
        return Ok(());
    }

    CliService::info(&format!("Syncing {} images...", source));

    std::fs::create_dir_all(&dest).context("Failed to create content images directory")?;

    copy_dir_recursive(&src, &dest)?;

    CliService::success(&format!("{} images synced", source));
    Ok(())
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src).context("Failed to read source directory")? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if path.is_dir() {
            std::fs::create_dir_all(&dest_path)?;
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path).context("Failed to copy file")?;
        }
    }
    Ok(())
}
