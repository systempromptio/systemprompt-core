use anyhow::Result;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Password, Select};
use std::path::Path;
use systemprompt_cloud::ProfilePath;
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

pub async fn edit_api_keys(profile_dir: &Path) -> Result<()> {
    CliService::section("API Keys (secrets.json)");

    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);

    if !secrets_path.exists() {
        CliService::warning("No secrets.json found in this profile.");
        return Ok(());
    }

    let content = std::fs::read_to_string(&secrets_path)?;
    let mut secrets: serde_json::Value = serde_json::from_str(&content)?;

    let key_options = vec![
        "Gemini API Key",
        "Anthropic API Key",
        "OpenAI API Key",
        "Database URL",
        "Done",
    ];

    loop {
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select key to edit")
            .items(&key_options)
            .default(0)
            .interact()?;

        match selection {
            0 => edit_gemini_key(&mut secrets)?,
            1 => edit_anthropic_key(&mut secrets)?,
            2 => edit_openai_key(&mut secrets)?,
            3 => edit_database_url(&mut secrets)?,
            4 => break,
            _ => unreachable!(),
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

fn edit_gemini_key(secrets: &mut serde_json::Value) -> Result<()> {
    let current = secrets.get("gemini").and_then(|v| v.as_str()).unwrap_or("");
    let masked = if current.is_empty() {
        "(not set)"
    } else {
        "***"
    };
    CliService::info(&format!("Current: {}", masked));

    let new_key = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("New Gemini API Key (empty to skip)")
        .allow_empty_password(true)
        .interact()?;

    if !new_key.is_empty() {
        secrets["gemini"] = serde_json::Value::String(new_key);
    }
    Ok(())
}

fn edit_anthropic_key(secrets: &mut serde_json::Value) -> Result<()> {
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

    let new_key = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("New Anthropic API Key (empty to skip)")
        .allow_empty_password(true)
        .interact()?;

    if !new_key.is_empty() {
        secrets["anthropic"] = serde_json::Value::String(new_key);
    }
    Ok(())
}

fn edit_openai_key(secrets: &mut serde_json::Value) -> Result<()> {
    let current = secrets.get("openai").and_then(|v| v.as_str()).unwrap_or("");
    let masked = if current.is_empty() {
        "(not set)"
    } else {
        "***"
    };
    CliService::info(&format!("Current: {}", masked));

    let new_key = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("New OpenAI API Key (empty to skip)")
        .allow_empty_password(true)
        .interact()?;

    if !new_key.is_empty() {
        secrets["openai"] = serde_json::Value::String(new_key);
    }
    Ok(())
}

fn edit_database_url(secrets: &mut serde_json::Value) -> Result<()> {
    let current = secrets
        .get("database_url")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    CliService::info(&format!("Current: {}", Profile::mask_database_url(current)));

    let new_url: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("New Database URL (empty to skip)")
        .allow_empty(true)
        .interact_text()?;

    if !new_url.is_empty() {
        secrets["database_url"] = serde_json::Value::String(new_url);
    }
    Ok(())
}
