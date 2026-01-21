use anyhow::{bail, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Password, Select};
use systemprompt_logging::CliService;

#[derive(Debug)]
pub struct ApiKeys {
    pub gemini: Option<String>,
    pub anthropic: Option<String>,
    pub openai: Option<String>,
}

pub fn collect_api_keys() -> Result<ApiKeys> {
    CliService::info("At least one AI provider API key is required.");

    let providers = vec![
        "Google AI (Gemini) - https://aistudio.google.com/app/apikey",
        "Anthropic (Claude) - https://console.anthropic.com/api-keys",
        "OpenAI (GPT) - https://platform.openai.com/api-keys",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select your AI provider")
        .items(&providers)
        .default(0)
        .interact()?;

    match selection {
        0 => {
            let key = Password::with_theme(&ColorfulTheme::default())
                .with_prompt("Gemini API Key")
                .interact()?;
            if key.is_empty() {
                bail!("API key is required");
            }
            Ok(ApiKeys {
                gemini: Some(key),
                anthropic: None,
                openai: None,
            })
        },
        1 => {
            let key = Password::with_theme(&ColorfulTheme::default())
                .with_prompt("Anthropic API Key")
                .interact()?;
            if key.is_empty() {
                bail!("API key is required");
            }
            Ok(ApiKeys {
                gemini: None,
                anthropic: Some(key),
                openai: None,
            })
        },
        2 => {
            let key = Password::with_theme(&ColorfulTheme::default())
                .with_prompt("OpenAI API Key")
                .interact()?;
            if key.is_empty() {
                bail!("API key is required");
            }
            Ok(ApiKeys {
                gemini: None,
                anthropic: None,
                openai: Some(key),
            })
        },
        _ => bail!("Invalid selection"),
    }
}
