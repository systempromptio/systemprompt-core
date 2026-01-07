//! Cloud logout command

use anyhow::Result;
use systemprompt_cloud::{get_cloud_paths, CloudPath};
use systemprompt_core_logging::CliService;

pub fn execute() -> Result<()> {
    let cloud_paths = get_cloud_paths()?;
    let creds_path = cloud_paths.resolve(CloudPath::Credentials);

    if !creds_path.exists() {
        CliService::success("Already logged out (no credentials found)");
        return Ok(());
    }

    std::fs::remove_file(&creds_path)?;
    CliService::key_value(
        "Removed credentials from",
        &creds_path.display().to_string(),
    );
    CliService::success("Logged out of SystemPrompt Cloud");

    Ok(())
}
