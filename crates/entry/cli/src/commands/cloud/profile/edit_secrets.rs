use anyhow::Result;
use std::path::Path;
use systemprompt_cloud::ProfilePath;
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

use crate::interactive::Prompter;

pub(super) fn edit_api_keys(prompter: &dyn Prompter, profile_dir: &Path) -> Result<()> {
    CliService::section("API Keys (secrets.json)");

    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);

    if !secrets_path.exists() {
        CliService::warning("No secrets.json found in this profile.");
        return Ok(());
    }

    let content = std::fs::read_to_string(&secrets_path)?;
    // JSON: round-trips the operator-authored secrets document so a single key
    // can be edited in place without dropping unknown fields.
    let mut secrets: serde_json::Value = serde_json::from_str(&content)?;

    let key_options: Vec<String> = [
        "Gemini API Key",
        "Anthropic API Key",
        "OpenAI API Key",
        "Database URL",
        "Done",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();

    loop {
        let selection = prompter.select("Select key to edit", &key_options)?;

        match selection {
            0 => edit_gemini_key(prompter, &mut secrets)?,
            1 => edit_anthropic_key(prompter, &mut secrets)?,
            2 => edit_openai_key(prompter, &mut secrets)?,
            3 => edit_database_url(prompter, &mut secrets)?,
            4 => break,
            other => return Err(anyhow::anyhow!("unexpected menu selection: {other}")),
        }
    }

    let content = serde_json::to_string_pretty(&secrets)?;
    std::fs::write(&secrets_path, content)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&secrets_path, permissions)?;
    }

    CliService::success("API keys updated");
    Ok(())
}

fn edit_gemini_key(prompter: &dyn Prompter, secrets: &mut serde_json::Value) -> Result<()> {
    let current = secrets.get("gemini").and_then(|v| v.as_str()).unwrap_or("");
    let masked = if current.is_empty() {
        "(not set)"
    } else {
        "***"
    };
    CliService::info(&format!("Current: {}", masked));

    let new_key = prompter.password("New Gemini API Key (empty to skip)")?;

    if !new_key.is_empty() {
        secrets["gemini"] = serde_json::Value::String(new_key);
    }
    Ok(())
}

fn edit_anthropic_key(prompter: &dyn Prompter, secrets: &mut serde_json::Value) -> Result<()> {
    let current = secrets
        .get("anthropic")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let masked = if current.is_empty() {
        "(not set)"
    } else {
        "***"
    };
    CliService::info(&format!("Current: {}", masked));

    let new_key = prompter.password("New Anthropic API Key (empty to skip)")?;

    if !new_key.is_empty() {
        secrets["anthropic"] = serde_json::Value::String(new_key);
    }
    Ok(())
}

fn edit_openai_key(prompter: &dyn Prompter, secrets: &mut serde_json::Value) -> Result<()> {
    let current = secrets.get("openai").and_then(|v| v.as_str()).unwrap_or("");
    let masked = if current.is_empty() {
        "(not set)"
    } else {
        "***"
    };
    CliService::info(&format!("Current: {}", masked));

    let new_key = prompter.password("New OpenAI API Key (empty to skip)")?;

    if !new_key.is_empty() {
        secrets["openai"] = serde_json::Value::String(new_key);
    }
    Ok(())
}

fn edit_database_url(prompter: &dyn Prompter, secrets: &mut serde_json::Value) -> Result<()> {
    let current = secrets
        .get("database_url")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    CliService::info(&format!("Current: {}", Profile::mask_database_url(current)));

    let new_url = prompter.input("New Database URL (empty to skip)")?;

    if !new_url.is_empty() {
        secrets["database_url"] = serde_json::Value::String(new_url);
    }
    Ok(())
}
